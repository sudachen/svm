use crate::{Address, Layer, SectionKind, SectionLike, TemplateAddr, TransactionId};

/// Stores data related to the deployment of a `Template`
#[derive(Debug, Clone, PartialEq)]
pub struct DeploySection {
    tx_id: TransactionId,
    layer: Layer,
    principal: Address,
    template: TemplateAddr,
}

impl DeploySection {
    /// Creates a new `Section`
    pub fn new(
        tx_id: TransactionId,
        layer: Layer,
        principal: Address,
        template: TemplateAddr,
    ) -> Self {
        Self {
            tx_id,
            layer,
            principal,
            template,
        }
    }

    /// The [`TransactionId`] of the `Deploy Template` transaction.
    pub fn tx_id(&self) -> &TransactionId {
        &self.tx_id
    }

    /// The [`Layer`] at which the [`Template`](crate::Template) has been deployed at.
    pub fn layer(&self) -> Layer {
        self.layer
    }

    /// The `Address` of the transaction's principal.
    pub fn principal(&self) -> &Address {
        &self.principal
    }

    /// The `Address` of the deployed [`Template`](crate::Template).
    pub fn template(&self) -> &TemplateAddr {
        &self.template
    }
}

impl SectionLike for DeploySection {
    const KIND: SectionKind = SectionKind::Deploy;
}
