mod meta;

use clap::ArgMatches;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use std::fs::File;
use std::io::Write;

use svm_codec::{api::builder::TemplateBuilder, SectionsEncoder};
use svm_layout::{FixedLayout, FixedLayoutBuilder, Id, Layout};
use svm_types::{CodeSection, CtorsSection, DataSection, Section, Sections};

use meta::TemplateMeta;

pub fn clap_app_craft_deploy() -> clap::App<'static, 'static> {
    use clap::*;

    SubCommand::with_name("craft-deploy")
        .about("High-level API to craft \"Deploy\" transactions")
        .arg(
            Arg::with_name("smwasm")
                .help("Path to the smWasm `#[template]` code")
                .long("smwasm")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("meta")
                .help("Path to the JSON meta-information produced by the SVM SDK")
                .long("meta")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("output")
                .help("Writes the binary output to this file")
                .short("o")
                .long("output")
                .required(true)
                .takes_value(true),
        )
}

pub fn subcmd_craft_deploy(args: &ArgMatches) -> anyhow::Result<()> {
    let smwasm = {
        let path = args.value_of("smwasm").unwrap();
        std::fs::read(path)?
    };
    let meta: TemplateMeta = {
        let path = args.value_of("meta").unwrap();
        let string = std::fs::read_to_string(path)?;
        serde_json::from_str(string.as_str())?
    };

    let flags = CodeSection::exec_flags();
    let code_section = CodeSection::new(
        svm_types::CodeKind::Wasm,
        smwasm,
        flags,
        svm_types::GasMode::Fixed,
        0,
    );

    let mut sections = Sections::with_capacity(3);
    sections.insert(Section::Code(code_section));
    sections.insert(Section::Ctors(meta.ctors_section()));
    sections.insert(Section::Data(meta.data_section()));

    let mut encoder = SectionsEncoder::with_capacity(3);
    encoder.encode(&sections);
    let bytes = encoder.finish();

    let mut file = File::create(args.value_of("output").unwrap())?;
    file.write_all(&bytes)?;
    Ok(())
}
