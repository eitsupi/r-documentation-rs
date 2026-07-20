#[cfg(feature = "gzip")]
use std::path::PathBuf;

#[cfg(feature = "gzip")]
use rd_rds::package::{PackageMeta, ViewError};
#[cfg(feature = "gzip")]
use rd_rds::{Attribute, Attributes, RObject, RValue};

#[cfg(feature = "gzip")]
fn fixture() -> RObject {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data/package-meta/fixturepkg-package.rds");
    rd_rds::file::read(path).expect("package metadata fixture")
}

#[cfg(feature = "gzip")]
#[test]
fn reads_package_metadata() {
    let object = fixture();
    let metadata = PackageMeta::try_from(&object).expect("package metadata");
    assert_eq!(
        metadata.description_field("Package"),
        Some(Some("fixturepkg"))
    );
    assert_eq!(metadata.description_field("Version"), Some(Some("0.1.0")));
    let built = metadata.built().expect("Built element");
    assert_eq!(built.r_version().to_string(), "4.6.1");
    assert_eq!(built.platform(), Some("x86_64-pc-linux-gnu"));
    assert_eq!(built.date(), Some("2026-06-24 19:14:59 UTC"));
    assert_eq!(built.os_type(), Some("unix"));
}

#[cfg(feature = "gzip")]
#[test]
fn validates_root_class() {
    let mut object = fixture();
    let attributes = without_attribute(object.attributes(), "class");
    set_attributes(&mut object, attributes);
    let error = PackageMeta::try_from(&object).unwrap_err();
    assert_eq!(error.path(), "PackageMeta.class");
}

#[cfg(feature = "gzip")]
#[test]
fn distinguishes_absent_and_malformed_built() {
    let object = fixture();
    let mut without_built = object.clone();
    remove_list_element(&mut without_built, "Built");
    assert!(
        PackageMeta::try_from(&without_built)
            .unwrap()
            .built()
            .is_none()
    );

    let mut missing_r = object.clone();
    let built = list_element(&missing_r, "Built").clone();
    let mut built = built;
    remove_list_element(&mut built, "R");
    replace_list_element(&mut missing_r, "Built", built);
    let error = PackageMeta::try_from(&missing_r).unwrap_err();
    assert_eq!(error.path(), "PackageMeta.Built.R");
}

#[cfg(feature = "gzip")]
#[test]
fn rejects_invalid_versions_and_duplicate_description_names() {
    let object = fixture();

    let mut wrong_class = object.clone();
    let mut version = list_element(&wrong_class, "Built").clone();
    let r = list_element(&version, "R").clone();
    let mut r = r;
    set_attributes(&mut r, Attributes::default());
    replace_list_element(&mut version, "R", r);
    replace_list_element(&mut wrong_class, "Built", version);
    assert!(matches!(
        PackageMeta::try_from(&wrong_class),
        Err(ViewError::InvalidPackageVersion { .. })
    ));

    let mut bad_component = object.clone();
    let mut version = list_element(&bad_component, "Built").clone();
    let r = list_element(&version, "R").clone();
    let (value, attributes) = r.into_parts();
    let RValue::List(mut values) = value else {
        panic!("R version list")
    };
    let (component_value, component_attributes) = values[0].clone().into_parts();
    let RValue::Integer(mut components) = component_value else {
        panic!("version integers")
    };
    components[0] = Some(-1);
    values[0] = RObject::from_parts(RValue::Integer(components), component_attributes);
    let r = RObject::from_parts(RValue::List(values), attributes);
    replace_list_element(&mut version, "R", r);
    replace_list_element(&mut bad_component, "Built", version);
    assert!(matches!(
        PackageMeta::try_from(&bad_component),
        Err(ViewError::InvalidPackageVersion { .. })
    ));

    let mut na_component = object.clone();
    let mut version = list_element(&na_component, "Built").clone();
    let r = list_element(&version, "R").clone();
    let (value, attributes) = r.into_parts();
    let RValue::List(mut values) = value else {
        panic!("R version list")
    };
    let (component_value, component_attributes) = values[0].clone().into_parts();
    let RValue::Integer(mut components) = component_value else {
        panic!("version integers")
    };
    components[0] = None;
    values[0] = RObject::from_parts(RValue::Integer(components), component_attributes);
    let r = RObject::from_parts(RValue::List(values), attributes);
    replace_list_element(&mut version, "R", r);
    replace_list_element(&mut na_component, "Built", version);
    assert!(matches!(
        PackageMeta::try_from(&na_component),
        Err(ViewError::InvalidPackageVersion { .. })
    ));

    let mut duplicate = object.clone();
    let mut description = list_element(&duplicate, "DESCRIPTION").clone();
    let names = description
        .attributes()
        .get("names")
        .expect("DESCRIPTION names")
        .clone();
    let RValue::Character(values) = names.value() else {
        panic!("DESCRIPTION names")
    };
    let mut values = values.clone();
    values[1] = values[0].clone();
    replace_attribute(
        &mut description,
        "names",
        RObject::from_parts(RValue::Character(values), Attributes::default()),
    );
    replace_list_element(&mut duplicate, "DESCRIPTION", description);
    let error = PackageMeta::try_from(&duplicate).unwrap_err();
    assert!(matches!(error, ViewError::DuplicateName { .. }));
    assert_eq!(error.field(), Some("Package"));
}

#[cfg(feature = "gzip")]
#[test]
fn preserves_description_na_and_supports_consumer_checks() {
    let mut object = fixture();
    let description = list_element(&object, "DESCRIPTION").clone();
    let (value, attributes) = description.into_parts();
    let RValue::Character(mut values) = value else {
        panic!("DESCRIPTION values")
    };
    values[0] = rd_rds::RStr::Na;
    let description = RObject::from_parts(RValue::Character(values), attributes);
    replace_list_element(&mut object, "DESCRIPTION", description);
    let metadata = PackageMeta::try_from(&object).unwrap();
    assert_eq!(metadata.description_field("Package"), Some(None));
    let version = metadata.built().unwrap().r_version().components();
    assert!(version.len() >= 2 && version[0] >= 4);
    assert!(metadata.built().unwrap().platform().is_some());
}

#[cfg(feature = "gzip")]
fn list_element<'a>(object: &'a RObject, name: &str) -> &'a RObject {
    object.get_named(name).expect("named list element")
}

#[cfg(feature = "gzip")]
fn replace_list_element(object: &mut RObject, name: &str, replacement: RObject) {
    let index = object
        .names()
        .unwrap()
        .iter()
        .position(|value| value.as_str().and_then(Result::ok).as_deref() == Some(name))
        .unwrap();
    let (value, attributes) = object.clone().into_parts();
    let RValue::List(mut values) = value else {
        panic!("list")
    };
    values[index] = replacement;
    *object = RObject::from_parts(RValue::List(values), attributes);
}

#[cfg(feature = "gzip")]
fn remove_list_element(object: &mut RObject, name: &str) {
    let index = object
        .names()
        .unwrap()
        .iter()
        .position(|value| value.as_str().and_then(Result::ok).as_deref() == Some(name))
        .unwrap();
    let (value, attributes) = object.clone().into_parts();
    let RValue::List(mut values) = value else {
        panic!("list")
    };
    values.remove(index);
    *object = RObject::from_parts(RValue::List(values), attributes);
    let names = object.attributes().get("names").unwrap().clone();
    let RValue::Character(mut names) = names.value().clone() else {
        panic!("names")
    };
    names.remove(index);
    replace_attribute(
        object,
        "names",
        RObject::from_parts(RValue::Character(names), Attributes::default()),
    );
}

#[cfg(feature = "gzip")]
fn without_attribute(attributes: &Attributes, name: &str) -> Attributes {
    Attributes::new(
        attributes
            .iter()
            .filter(|attribute| attribute.name().as_str() != name)
            .cloned()
            .collect(),
    )
}

#[cfg(feature = "gzip")]
fn replace_attribute(object: &mut RObject, name: &str, value: RObject) {
    let attributes = object
        .attributes()
        .iter()
        .map(|attribute| {
            if attribute.name().as_str() == name {
                Attribute::new(attribute.name().clone(), value.clone())
            } else {
                attribute.clone()
            }
        })
        .collect();
    set_attributes(object, Attributes::new(attributes));
}

#[cfg(feature = "gzip")]
fn set_attributes(object: &mut RObject, attributes: Attributes) {
    let (value, _) = object.clone().into_parts();
    *object = RObject::from_parts(value, attributes);
}
