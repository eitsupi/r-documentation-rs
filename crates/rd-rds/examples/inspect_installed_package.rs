//! Inspect declared namespace metadata and stored code bindings without an R session.
//!
//! Run with:
//!
//! ```text
//! cargo run -p rd-rds --features lazyload --example inspect_installed_package -- /path/to/installed/package [binding]
//! ```
//!
//! This is a small CLI demonstration, not a runtime namespace inspection
//! product: it reads static metadata and the installed code database only.

use std::{env, error::Error, path::PathBuf};

use rd_rds::package::{
    DefaultPresence, FormalsInspection, FormalsNotApplicable, FormalsUnavailable, InstalledCodeDb,
    MetadataField, NamespaceMetadata, StoredObjectInspection,
};

struct SignatureFormal {
    name: String,
    default: DefaultPresence,
}

enum SignatureView {
    Available(Vec<SignatureFormal>),
    NotApplicable(&'static str),
    Unavailable(String),
}

fn signature_summary(view: SignatureView) -> String {
    match view {
        SignatureView::Available(formals) => {
            let entries = formals
                .iter()
                .map(|formal| format!("{} ({:?})", formal.name, formal.default))
                .collect::<Vec<_>>();
            format!("available [{}]", entries.join(", "))
        }
        SignatureView::NotApplicable(reason) => format!("not applicable ({reason})"),
        SignatureView::Unavailable(reason) => format!("unavailable ({reason})"),
    }
}

fn signature_view(inspection: &StoredObjectInspection) -> SignatureView {
    match inspection.formals() {
        FormalsInspection::Available(formals) => SignatureView::Available(
            formals
                .iter()
                .map(|formal| SignatureFormal {
                    name: formal.name().to_owned(),
                    default: formal.default(),
                })
                .collect(),
        ),
        FormalsInspection::NotApplicable(FormalsNotApplicable::BuiltIn) => {
            SignatureView::NotApplicable("built-in")
        }
        FormalsInspection::NotApplicable(FormalsNotApplicable::Special) => {
            SignatureView::NotApplicable("special")
        }
        FormalsInspection::NotApplicable(FormalsNotApplicable::NonClosure) => {
            SignatureView::NotApplicable("non-closure")
        }
        FormalsInspection::Unavailable(FormalsUnavailable::PromiseNotEvaluated) => {
            SignatureView::Unavailable("promise was not evaluated".to_owned())
        }
        FormalsInspection::Unavailable(FormalsUnavailable::PersistentReferenceUnresolved) => {
            SignatureView::Unavailable("persistent reference was not resolved".to_owned())
        }
        FormalsInspection::Unavailable(FormalsUnavailable::Prefix(failure)) => {
            SignatureView::Unavailable(format!(
                "prefix failure at {:?} byte {} ({:?})",
                failure.phase(),
                failure.offset(),
                failure.cause()
            ))
        }
        _ => SignatureView::Unavailable("formals state is unknown".to_owned()),
    }
}

fn print_s3_evidence(metadata: &NamespaceMetadata) {
    print!("Declared S3 generic evidence: ");
    match metadata.s3_generic_evidence() {
        MetadataField::Present(generics) => println!("Present {generics:?}"),
        MetadataField::Missing => println!("Missing"),
        MetadataField::Invalid(error) => println!("Invalid ({error})"),
        MetadataField::UnsupportedSchema { description } => {
            println!("Unsupported schema ({description})")
        }
        _ => println!("Unknown state"),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args().skip(1);
    let package_dir = arguments
        .next()
        .map(PathBuf::from)
        .ok_or("usage: inspect_installed_package <installed-package-dir> [binding]")?;
    let binding = arguments.next();
    if arguments.next().is_some() {
        return Err("usage: inspect_installed_package <installed-package-dir> [binding]".into());
    }

    println!("Installed package directory: {}", package_dir.display());
    println!("Declared namespace metadata (static; no R runtime is started):");
    let namespace_object = rd_rds::file::read(package_dir.join("Meta/nsInfo.rds"))?;
    let namespace = NamespaceMetadata::from_object(&namespace_object)?;
    print_s3_evidence(&namespace);

    let database = InstalledCodeDb::open(&package_dir)?;
    println!("Stored code-database bindings (CodeDatabaseVariables; not runtime bindings):");
    for stored in database.stored_bindings() {
        println!("  {}", stored.name());
    }

    if let Some(name) = binding {
        let inspection = database.inspect_stored_binding(&name)?;
        println!("Binding {name:?}: kind={:?}", inspection.kind());
        println!(
            "  formals policy view: {}",
            signature_summary(signature_view(&inspection))
        );
        println!("  extent: {:?}", inspection.extent());
    }

    println!("Runtime namespace: not started or inspected");
    Ok(())
}
