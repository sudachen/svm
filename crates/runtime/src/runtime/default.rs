use log::info;
use wasmer::{Instance, Module, WasmPtr, WasmTypeList};

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use svm_gas::FuncPrice;
use svm_layout::FixedLayout;
use svm_program::Program;
use svm_storage::account::AccountStorage;
use svm_types::{
    Address, CallReceipt, Context, DeployReceipt, Envelope, Gas, GasMode, OOGError, ReceiptLog,
    RuntimeError, SectionKind, SpawnReceipt, State, Template, TemplateAddr, Transaction,
};

use super::{Call, Failure, Function, Outcome};
use crate::env::{EnvTypes, ExtAccount, ExtSpawn};
use crate::error::ValidateError;
use crate::storage::StorageBuilderFn;
use crate::Env;
use crate::{vmcalls, ProtectedMode};
use crate::{Config, FuncEnv, Runtime};

type Result<T> = std::result::Result<Outcome<T>, Failure>;

/// Default [`Runtime`] implementation based on [`Wasmer`](https://wasmer.io).
pub struct DefaultRuntime<T>
where
    T: EnvTypes,
{
    /// Runtime Environment.
    ///
    /// Used mainly for managing an Account's persistence.
    env: Env<T>,

    /// Provided host functions to be consumed by running transactions.
    imports: (String, wasmer::Exports),

    /// Runtime configuration.
    config: Config,

    /// Builds an `AccountStorage` instance.
    storage_builder: Box<StorageBuilderFn>,

    /// A naive cache for [`Template`]s' [`FuncPrice`]s. The cache key will, in
    /// the future, also include an identifier for which
    /// [`PriceResolver`](svm_gas::PriceResolver) should be used (possibly an
    /// `u16`?).
    template_prices: Rc<RefCell<HashMap<TemplateAddr, FuncPrice>>>,
}

impl<T> DefaultRuntime<T>
where
    T: EnvTypes,
{
    /// Initializes a new [`DefaultRuntime`].
    ///
    /// `template_prices` offers an easy way to inject an append-only, naive caching mechanism to
    /// the [`Template`] pricing logic; using a `None` will result in a new
    /// empty cache and on-the-fly calculation for all [`Template`]s.
    pub fn new(
        env: Env<T>,
        imports: (String, wasmer::Exports),
        storage_builder: Box<StorageBuilderFn>,
        config: Config,
        template_prices: Option<Rc<RefCell<HashMap<TemplateAddr, FuncPrice>>>>,
    ) -> Self {
        let template_prices = if let Some(tp) = template_prices {
            tp
        } else {
            Rc::new(RefCell::new(HashMap::default()))
        };
        Self {
            env,
            imports,
            storage_builder,
            config,
            template_prices,
        }
    }

    fn outcome_to_receipt(
        &self,
        env: &FuncEnv,
        mut out: Outcome<Box<[wasmer::Val]>>,
    ) -> CallReceipt {
        CallReceipt {
            version: 0,
            success: true,
            error: None,
            returndata: Some(self.take_returndata(env)),
            new_state: Some(self.commit_changes(&env)),
            gas_used: out.gas_used(),
            logs: out.take_logs(),
        }
    }

    fn failure_to_receipt(&self, mut fail: Failure) -> CallReceipt {
        let logs = fail.take_logs();
        let err = fail.take_error();

        CallReceipt::from_err(err, logs)
    }

    /// Opens the [`AccountStorage`] associated with the input parameters.
    pub fn open_storage(
        &self,
        target: &Address,
        state: &State,
        layout: &FixedLayout,
    ) -> AccountStorage {
        (self.storage_builder)(target, state, layout, &self.config)
    }

    fn call_ctor(
        &mut self,
        spawn: &ExtSpawn,
        target: Address,
        gas_left: Gas,
        envelope: &Envelope,
        context: &Context,
    ) -> SpawnReceipt {
        let template = spawn.template_addr().clone();

        let call = Call {
            func_name: spawn.ctor_name(),
            func_input: spawn.ctor_data(),
            state: &State::zeros(),
            template,
            target: target.clone(),
            within_spawn: true,
            gas_limit: gas_left,
            protected_mode: ProtectedMode::FullAccess,
            envelope,
            context,
        };

        let receipt = self.exec_call::<(), ()>(&call);

        // TODO: move the `into_spawn_receipt` to a `From / TryFrom`
        svm_types::into_spawn_receipt(receipt, &target)
    }

    fn exec_call<Args, Rets>(&mut self, call: &Call) -> CallReceipt {
        let result = self.exec::<(), (), _, _>(&call, |env, out| self.outcome_to_receipt(env, out));

        result.unwrap_or_else(|fail| self.failure_to_receipt(fail))
    }

    fn exec<Args, Rets, F, R>(&self, call: &Call, f: F) -> std::result::Result<R, Failure>
    where
        Args: WasmTypeList,
        Rets: WasmTypeList,
        F: Fn(&FuncEnv, Outcome<Box<[wasmer::Val]>>) -> R,
    {
        match self.account_template(&call.target) {
            Ok(template) => {
                let storage = self.open_storage(&call.target, call.state, template.fixed_layout());

                let mut env = FuncEnv::new(
                    storage,
                    call.envelope,
                    call.context,
                    call.template.clone(),
                    call.target.clone(),
                    call.protected_mode,
                );

                let store = crate::wasm_store::new_store();
                let import_object = self.create_import_object(&store, &mut env);

                let res = self.run::<Args, Rets>(&call, &store, &env, &template, &import_object);
                res.map(|rets| f(&env, rets))
            }
            Err(err) => Err(err.into()),
        }
    }

    fn run<Args, Rets>(
        &self,
        call: &Call,
        store: &wasmer::Store,
        func_env: &FuncEnv,
        template: &Template,
        import_object: &wasmer::ImportObject,
    ) -> Result<Box<[wasmer::Val]>>
    where
        Args: WasmTypeList,
        Rets: WasmTypeList,
    {
        self.validate_call(call, template, func_env)?;

        let module = self.compile_template(store, func_env, &template, call.gas_limit)?;
        let instance = self.instantiate(func_env, &module, import_object)?;

        self.set_memory(func_env, &instance);

        let func = self.func::<Args, Rets>(&instance, func_env, call.func_name)?;

        let mut out = if call.func_input.len() > 0 {
            self.call_with_alloc(&instance, func_env, call.func_input, &func, &[])?
        } else {
            self.wasmer_call(&instance, func_env, &func, &[])?
        };

        let logs = out.take_logs();

        match self.instance_gas_used(&instance) {
            Ok(gas_used) => {
                let returns = out.take_returns();
                let out = Outcome::new(returns, gas_used, logs);

                Ok(out)
            }
            Err(..) => {
                let err = Failure::new(RuntimeError::OOG, out.take_logs());
                Err(err)
            }
        }
    }

    fn call_with_alloc<Args, Rets>(
        &self,
        instance: &Instance,
        env: &FuncEnv,
        calldata: &[u8],
        func: &Function<Args, Rets>,
        params: &[wasmer::Val],
    ) -> Result<Box<[wasmer::Val]>>
    where
        Args: WasmTypeList,
        Rets: WasmTypeList,
    {
        debug_assert!(calldata.is_empty() == false);

        let out = self.call_alloc(instance, env, calldata.len())?;

        // we assert that `svm_alloc` didn't touch the `returndata`
        // TODO: return an error instead of `panic`
        self.assert_no_returndata(env);

        let wasm_ptr = out.returns();
        self.set_calldata(env, calldata, wasm_ptr);

        self.wasmer_call(instance, env, func, params)
    }

    fn call_alloc(&self, instance: &Instance, env: &FuncEnv, size: usize) -> Result<WasmPtr<u8>> {
        // Backups the current [`ProtectedMode`].
        let origin_mode = env.protected_mode();

        // Sets `Access Denied` mode while running `svm_alloc`.
        env.set_protected_mode(ProtectedMode::AccessDenied);

        let func_name = "svm_alloc";

        let func = self.func::<u32, u32>(&instance, env, func_name);
        if func.is_err() {
            // ### Notes:
            //
            // We don't restore the original [`ProtectedMode`]
            // since `svm_alloc` has failed and the transaction will halt.
            let err = self.func_not_found(env, func_name);
            return Err(err);
        }

        let func = func.unwrap();
        let params: [wasmer::Val; 1] = [(size as i32).into()];

        let out = self.wasmer_call(instance, env, &func, &params)?;
        let out = out.map(|rets| {
            let ret = &rets[0];
            let offset = ret.i32().unwrap() as u32;

            WasmPtr::new(offset)
        });

        // Restores the original [`ProtectedMode`].
        env.set_protected_mode(origin_mode);

        Ok(out)
    }

    fn wasmer_call<Args, Rets>(
        &self,
        instance: &Instance,
        env: &FuncEnv,
        func: &Function<Args, Rets>,
        params: &[wasmer::Val],
    ) -> Result<Box<[wasmer::Val]>>
    where
        Args: WasmTypeList,
        Rets: WasmTypeList,
    {
        let wasmer_func = func.wasmer_func();
        let returns = wasmer_func.call(params);
        let logs = env.borrow_mut().take_logs();

        if returns.is_err() {
            let err = self.func_failed(env, func.name(), returns.unwrap_err(), logs);
            return Err(err);
        }

        match self.instance_gas_used(&instance) {
            Ok(gas_used) => {
                let out = Outcome::new(returns.unwrap(), gas_used, logs);
                Ok(out)
            }
            Err(..) => {
                let err = Failure::new(RuntimeError::OOG, logs);
                Err(err)
            }
        }
    }

    #[inline]
    fn commit_changes(&self, env: &FuncEnv) -> State {
        let mut borrow = env.borrow_mut();
        let storage = borrow.storage_mut();
        storage.commit()
    }

    #[inline]
    fn assert_no_returndata(&self, env: &FuncEnv) {
        assert!(env.borrow().returndata().is_none())
    }

    fn take_returndata(&self, env: &FuncEnv) -> Vec<u8> {
        let data = env.borrow().returndata();

        match data {
            Some((offset, length)) => self.read_memory(env, offset, length),
            None => Vec::new(),
        }
    }

    fn read_memory(&self, env: &FuncEnv, offset: usize, length: usize) -> Vec<u8> {
        assert!(length > 0);

        let borrow = env.borrow();
        let memory = borrow.memory();

        let view = memory.view::<u8>();
        assert!(view.len() > offset + length - 1);

        let cells = &view[offset..(offset + length)];
        cells.iter().map(|c| c.get()).collect()
    }

    fn set_memory(&self, env: &FuncEnv, instance: &Instance) {
        // TODO: raise when no exported memory exists
        let memory = instance.exports.get_memory("memory").unwrap();

        env.borrow_mut().set_memory(memory.clone());
    }

    fn set_calldata(&self, env: &FuncEnv, calldata: &[u8], wasm_ptr: WasmPtr<u8>) {
        debug_assert!(calldata.is_empty() == false);

        let (offset, len) = {
            let borrow = env.borrow();
            let memory = borrow.memory();

            // Each WASM instance memory contains at least one `WASM Page`. (A `Page` size is 64KB)
            // The `len(calldata)` will be less than the `WASM Page` size.
            //
            // In any case, the `alloc_memory` is in charge of allocating enough memory
            // for the program to run (so we don't need to have any bounds-checking here).
            //
            // TODO: add to `validate_template` checking that `calldata` doesn't exceed ???
            // (we'll need to decide on a `calldata` limit).
            //
            // See [issue #140](https://github.com/spacemeshos/svm/issues/140)
            let offset = wasm_ptr.offset() as usize;
            let length = calldata.len();
            let view = memory.view::<u8>();

            // TODO: fail safely, instead of using `assert!`
            assert!(view.len() > offset + length - 1);

            let cells = &view[offset..(offset + length)];
            for (cell, &byte) in cells.iter().zip(calldata.iter()) {
                cell.set(byte);
            }

            (offset, length)
        };

        env.borrow_mut().set_calldata(offset, len);
    }

    /// Calculates the amount of gas used by `instance`.
    #[inline]
    fn instance_gas_used(&self, _instance: &Instance) -> std::result::Result<Gas, OOGError> {
        // TODO: read `gas_used` out of `instance`
        Ok(Gas::new())
    }

    fn instantiate(
        &self,
        env: &FuncEnv,
        module: &Module,
        import_object: &wasmer::ImportObject,
    ) -> std::result::Result<Instance, Failure> {
        info!("Runtime `instantiate` (using Wasmer `Instance#new`)");

        let instance = Instance::new(module, import_object);
        instance.map_err(|err| self.instantiation_failed(env, err))
    }

    fn func<'i, Args, Rets>(
        &self,
        instance: &'i Instance,
        env: &FuncEnv,
        func_name: &'i str,
    ) -> std::result::Result<Function<'i, Args, Rets>, Failure>
    where
        Args: WasmTypeList,
        Rets: WasmTypeList,
    {
        let func = instance.exports.get_function(func_name);
        if func.is_err() {
            let err = self.func_not_found(env, func_name);
            return Err(err);
        }

        let func = func.unwrap();
        let native = func.native::<Args, Rets>();

        if native.is_err() {
            let err = self.func_invalid_sig(env, func_name);
            return Err(err);
        }

        let func = Function::new(func, func_name);
        Ok(func)
    }

    fn create_import_object(
        &self,
        store: &wasmer::Store,
        env: &mut FuncEnv,
    ) -> wasmer::ImportObject {
        let mut import_object = wasmer::ImportObject::new();

        // Registering SVM internals
        let mut internals = wasmer::Exports::new();
        vmcalls::wasmer_register(store, env, &mut internals);
        import_object.register("svm", internals);

        // Registering the externals provided to the Runtime
        let (name, exports) = &self.imports;
        debug_assert_ne!(name, "svm");

        import_object.register(name, exports.clone());

        import_object
    }

    fn account_template(
        &self,
        account_addr: &Address,
    ) -> std::result::Result<Template, RuntimeError> {
        let mut interests = HashSet::new();
        interests.insert(SectionKind::Code);
        interests.insert(SectionKind::Data);
        interests.insert(SectionKind::Ctors);

        let template = self.env.account_template(account_addr, Some(interests));
        template.ok_or_else(|| RuntimeError::AccountNotFound(account_addr.clone()))
    }

    fn compile_template(
        &self,
        store: &wasmer::Store,
        env: &FuncEnv,
        template: &Template,
        gas_left: Gas,
    ) -> std::result::Result<Module, Failure> {
        let module_res = Module::from_binary(store, template.code());
        let _gas_left = gas_left.unwrap_or(0);

        module_res.map_err(|err| self.compilation_failed(env, err))
    }

    fn validate_call(
        &self,
        call: &Call,
        template: &Template,
        env: &FuncEnv,
    ) -> std::result::Result<(), Failure> {
        // TODO: validate there is enough gas for running the `Transaction`.
        // * verify
        // * call
        // * other factors

        let spawning = call.within_spawn;
        let ctor = template.is_ctor(call.func_name);

        if spawning && !ctor {
            let msg = "expected function to be a constructor";
            let err = self.func_not_allowed(env, call.func_name, msg);

            return Err(err);
        }

        if !spawning && ctor {
            let msg = "expected function to be a non-constructor";
            let err = self.func_not_allowed(env, call.func_name, msg);

            return Err(err);
        }

        Ok(())
    }

    fn build_call<'a>(
        &self,
        tx: &'a Transaction,
        envelope: &'a Envelope,
        context: &'a Context,
        protected_mode: ProtectedMode,
        func_name: &'a str,
        func_input: &'a [u8],
    ) -> Call<'a> {
        let target = tx.target();
        let template = self.env.resolve_template_addr(target);

        if let Some(template) = template {
            Call {
                func_name,
                func_input,
                target: target.clone(),
                template,
                state: context.state(),
                gas_limit: envelope.gas_limit(),
                protected_mode,
                within_spawn: false,
                envelope,
                context,
            }
        } else {
            unreachable!("Should have failed earlier when doing `validate_call`");
        }
    }

    /// Errors

    #[inline]
    fn func_not_found(&self, env: &FuncEnv, func_name: &str) -> Failure {
        RuntimeError::FuncNotFound {
            target: env.target_addr().clone(),
            template: env.template_addr().clone(),
            func: func_name.to_string(),
        }
        .into()
    }

    #[inline]
    fn instantiation_failed(&self, env: &FuncEnv, err: wasmer::InstantiationError) -> Failure {
        RuntimeError::InstantiationFailed {
            target: env.target_addr().clone(),
            template: env.template_addr().clone(),
            msg: err.to_string(),
        }
        .into()
    }

    #[inline]
    fn func_not_allowed(&self, env: &FuncEnv, func_name: &str, msg: &str) -> Failure {
        RuntimeError::FuncNotAllowed {
            target: env.target_addr().clone(),
            template: env.template_addr().clone(),
            func: func_name.to_string(),
            msg: msg.to_string(),
        }
        .into()
    }

    #[inline]
    fn func_invalid_sig(&self, env: &FuncEnv, func_name: &str) -> Failure {
        RuntimeError::FuncInvalidSignature {
            target: env.target_addr().clone(),
            template: env.template_addr().clone(),
            func: func_name.to_string(),
        }
        .into()
    }

    #[inline]
    fn func_failed(
        &self,
        env: &FuncEnv,
        func_name: &str,
        err: wasmer::RuntimeError,
        logs: Vec<ReceiptLog>,
    ) -> Failure {
        let err = RuntimeError::FuncFailed {
            target: env.target_addr().clone(),
            template: env.template_addr().clone(),
            func: func_name.to_string(),
            msg: err.to_string(),
        };

        Failure::new(err, logs)
    }

    #[inline]
    fn compilation_failed(&self, env: &FuncEnv, err: wasmer::CompileError) -> Failure {
        RuntimeError::CompilationFailed {
            target: env.target_addr().clone(),
            template: env.template_addr().clone(),
            msg: err.to_string(),
        }
        .into()
    }
}

impl<T> Runtime for DefaultRuntime<T>
where
    T: EnvTypes,
{
    fn validate_deploy(&self, message: &[u8]) -> std::result::Result<(), ValidateError> {
        let template = self.env.parse_deploy(message, None)?;
        let code = template.code();

        // Opcode and `svm_alloc` checks should only ever be run when deploying [`Template`]s.
        // There's no reason to also do it when spawning new `Account`
        // over already-validated [`Template`]s
        let program = Program::new(code, true).map_err(ValidateError::from)?;
        svm_gas::validate_wasm(&program, false).map_err(ValidateError::from)?;

        Ok(())
    }

    fn validate_spawn(&self, message: &[u8]) -> std::result::Result<(), ValidateError> {
        self.env
            .parse_spawn(message)
            .map(|_| ())
            .map_err(Into::into)
    }

    fn validate_call(&self, message: &[u8]) -> std::result::Result<(), ValidateError> {
        self.env
            .parse_call(message)
            .map(|_| ())
            .map_err(|e| e.into())
    }

    fn deploy(&mut self, envelope: &Envelope, message: &[u8], _context: &Context) -> DeployReceipt {
        info!("Runtime `deploy`");

        let template = self
            .env
            .parse_deploy(message, None)
            .expect("Should have called `validate_deploy` first");

        let gas_limit = envelope.gas_limit();
        let install_price = svm_gas::transaction::deploy(message);

        if gas_limit >= install_price {
            let gas_used = Gas::with(install_price);
            let addr = self.env.compute_template_addr(&template);
            self.env.store_template(&template, &addr);

            DeployReceipt::new(addr, gas_used)
        } else {
            DeployReceipt::new_oog()
        }
    }

    fn spawn(&mut self, envelope: &Envelope, message: &[u8], context: &Context) -> SpawnReceipt {
        // TODO: refactor this function (it has got a bit lengthy...)

        use svm_gas::ProgramPricing;
        use svm_program::ProgramVisitor;

        info!("Runtime `spawn`");

        let gas_limit = envelope.gas_limit();
        let base = self
            .env
            .parse_spawn(message)
            .expect("Should have called `validate_spawn` first");

        let template_addr = base.account.template_addr();

        // TODO: load only the `Sections` relevant for spawning
        let template = self
            .env
            .template(template_addr, None)
            .expect("Should have failed earlier when doing `validate_spawn`");

        let code_section = template.code_section();
        let code = code_section.code();
        let gas_mode = code_section.gas_mode();
        let program = Program::new(code, false).unwrap();

        // We're using a naive memoization mechanism: we only ever add, never
        // remove. This means there's no cache invalidation at all. We can
        // easily afford to do this because the number of templates that exist
        // at genesis is fixed and won't grow.
        let mut template_prices = self.template_prices.borrow_mut();
        let func_price = {
            if let Some(prices) = template_prices.get(&template_addr) {
                prices
            } else {
                let pricer = self.env.price_resolver();
                let program_pricing = ProgramPricing::new(pricer);
                let prices = program_pricing.visit(&program).unwrap();

                template_prices.insert(template_addr.clone(), prices);
                template_prices.get(template_addr).unwrap()
            }
        };

        let spawner = envelope.principal();
        let spawn = ExtSpawn::new(base, &spawner);

        if !template.is_ctor(spawn.ctor_name()) {
            // The [`Template`] is faulty.
            let account = ExtAccount::new(spawn.account(), &spawner);
            let account_addr = self.env.compute_account_addr(&spawn);
            return SpawnReceipt::from_err(
                RuntimeError::FuncNotAllowed {
                    target: account_addr,
                    template: account.template_addr().clone(),
                    func: spawn.ctor_name().to_string(),
                    msg: "The given function is not a `ctor`.".to_string(),
                },
                vec![],
            );
        }

        match gas_mode {
            GasMode::Fixed => {
                let ctor_func_index = program.exports().get(spawn.ctor_name()).unwrap();
                let price = func_price.get(ctor_func_index) as u64;
                if gas_limit <= price {
                    return SpawnReceipt::new_oog(vec![]);
                }
            }
            GasMode::Metering => unreachable!("Not supported yet... (TODO)"),
        }

        // We don't need this anymore!
        drop(template_prices);

        let payload_price = svm_gas::transaction::spawn(message);
        let gas_left = gas_limit - payload_price;

        match gas_left {
            Ok(gas_left) => {
                let account = ExtAccount::new(spawn.account(), &spawner);
                let target = self.env.compute_account_addr(&spawn);

                self.env.store_account(&account, &target);
                self.call_ctor(&spawn, target, gas_left, envelope, context)
            }
            Err(..) => SpawnReceipt::new_oog(Vec::new()),
        }
    }

    fn verify(&mut self, envelope: &Envelope, message: &[u8], context: &Context) -> CallReceipt {
        let tx = self
            .env
            .parse_call(message)
            .expect("Should have called `validate_call` first");

        // ### Important:
        //
        // Right now we disallow any `Storage` access while running `svm_verify`.
        // This hard restriction might be mitigated in future versions.
        //
        // In that case, the current behavior should be backward-compatible since
        // we could always executed `Access Denied` logic when partial `Storage` access will be allowed by SVM.
        let call = self.build_call(
            &tx,
            envelope,
            context,
            ProtectedMode::AccessDenied,
            "svm_verify",
            tx.verifydata(),
        );

        // TODO: override the `call.gas_limit` with `VERIFY_MAX_GAS`
        self.exec_call::<(), ()>(&call)
    }

    fn call(&mut self, envelope: &Envelope, message: &[u8], context: &Context) -> CallReceipt {
        let tx = self
            .env
            .parse_call(message)
            .expect("Should have called `validate_call` first");

        let call = self.build_call(
            &tx,
            envelope,
            context,
            ProtectedMode::FullAccess,
            tx.func_name(),
            tx.calldata(),
        );

        self.exec_call::<(), ()>(&call)
    }
}
