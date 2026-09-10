use super::{PackageVersion, PackagesMatrix, ViewError};
use crate::{
    Attribute, Attributes, NativeEncodingSource, REncoding, RObject, RStr, RValue, Symbol,
};

#[test]
fn package_version_displays_components() {
    let version = PackageVersion {
        components: vec![4, 6, 1],
    };
    assert_eq!(version.to_string(), "4.6.1");
    assert_eq!(version.components(), &[4, 6, 1]);
}

#[test]
fn view_error_exposes_logical_context() {
    let error = ViewError::DuplicateName {
        path: "PackageMeta.DESCRIPTION[\"Package\"]".to_owned(),
        field: Some("Package".to_owned()),
    };
    assert_eq!(error.path(), "PackageMeta.DESCRIPTION[\"Package\"]");
    assert_eq!(error.field(), Some("Package"));
    assert_eq!(error.row(), None);
    assert_eq!(error.column(), None);
}

fn matrix(dim: Vec<Option<i32>>, dimnames: Vec<RObject>, values: Vec<RStr>) -> RObject {
    RObject::from_parts(
        RValue::Character(values),
        Attributes::new(vec![
            Attribute::new(
                Symbol::new("dim"),
                RObject::from_parts(RValue::Integer(dim), Attributes::default()),
            ),
            Attribute::new(
                Symbol::new("dimnames"),
                RObject::from_parts(RValue::List(dimnames), Attributes::default()),
            ),
        ]),
    )
}

fn names(values: &[&str]) -> RObject {
    RObject::from_parts(
        RValue::Character(
            values
                .iter()
                .map(|value| {
                    RStr::new(
                        value.as_bytes(),
                        REncoding::Native,
                        NativeEncodingSource::Unknown,
                    )
                })
                .collect(),
        ),
        Attributes::default(),
    )
}

fn replace_dimnames(object: &mut RObject, replacement: Vec<RObject>) {
    let dim = object.attributes().get("dim").unwrap().clone();
    set_attributes(
        object,
        Attributes::new(vec![
            Attribute::new(Symbol::new("dim"), dim),
            Attribute::new(
                Symbol::new("dimnames"),
                RObject::from_parts(RValue::List(replacement), Attributes::default()),
            ),
        ]),
    );
}

fn set_attributes(object: &mut RObject, attributes: Attributes) {
    let (value, _) = object.clone().into_parts();
    *object = RObject::from_parts(value, attributes);
}

fn set_value(object: &mut RObject, value: RValue) {
    let (_, attributes) = object.clone().into_parts();
    *object = RObject::from_parts(value, attributes);
}

#[test]
fn packages_matrix_rejects_malformed_shape_and_names() {
    let valid = || {
        matrix(
            vec![Some(1), Some(1)],
            vec![names(&["row"]), names(&["Package"])],
            vec![RStr::new(
                b"x",
                REncoding::Native,
                NativeEncodingSource::Unknown,
            )],
        )
    };
    assert!(
        matches!(PackagesMatrix::from_object(&RObject::from_parts(RValue::Character(vec![]), Attributes::default())), Err(ViewError::Missing { path, .. }) if path == "PACKAGES.attributes.dim")
    );
    let mut object = valid();
    set_attributes(
        &mut object,
        Attributes::new(vec![Attribute::new(
            Symbol::new("dim"),
            RObject::from_parts(RValue::Character(vec![]), Attributes::default()),
        )]),
    );
    assert!(
        matches!(PackagesMatrix::from_object(&object), Err(ViewError::UnexpectedType { path, .. }) if path == "PACKAGES.attributes.dim")
    );
    let mut object = valid();
    set_attributes(
        &mut object,
        Attributes::new(vec![Attribute::new(
            Symbol::new("dim"),
            RObject::from_parts(RValue::Integer(vec![Some(1)]), Attributes::default()),
        )]),
    );
    assert!(
        matches!(PackagesMatrix::from_object(&object), Err(ViewError::UnexpectedLength { path, .. }) if path == "PACKAGES.attributes.dim")
    );
    let mut object = valid();
    set_attributes(
        &mut object,
        Attributes::new(vec![
            Attribute::new(
                Symbol::new("dim"),
                RObject::from_parts(RValue::Integer(vec![None, Some(1)]), Attributes::default()),
            ),
            Attribute::new(
                Symbol::new("dimnames"),
                RObject::from_parts(
                    RValue::List(vec![names(&["row"]), names(&["Package"])]),
                    Attributes::default(),
                ),
            ),
        ]),
    );
    assert!(
        matches!(PackagesMatrix::from_object(&object), Err(ViewError::InvalidDimensions { path, .. }) if path == "PACKAGES.attributes.dim")
    );
    let mut object = valid();
    set_attributes(
        &mut object,
        Attributes::new(vec![
            Attribute::new(
                Symbol::new("dim"),
                RObject::from_parts(
                    RValue::Integer(vec![Some(-1), Some(1)]),
                    Attributes::default(),
                ),
            ),
            Attribute::new(
                Symbol::new("dimnames"),
                RObject::from_parts(
                    RValue::List(vec![names(&[]), names(&["Package"])]),
                    Attributes::default(),
                ),
            ),
        ]),
    );
    assert!(
        matches!(PackagesMatrix::from_object(&object), Err(ViewError::InvalidDimensions { path, .. }) if path == "PACKAGES.attributes.dim")
    );
    let mut object = valid();
    replace_dimnames(
        &mut object,
        vec![names(&["row"]), names(&["Package", "Version"])],
    );
    assert!(
        matches!(PackagesMatrix::from_object(&object), Err(ViewError::UnexpectedLength { path, .. }) if path == "PACKAGES.attributes.dimnames[1]")
    );
    let mut object = valid();
    replace_dimnames(&mut object, vec![names(&["row"]), names(&["Package"])]);
    set_value(&mut object, RValue::Character(vec![]));
    assert!(
        matches!(PackagesMatrix::from_object(&object), Err(ViewError::UnexpectedLength { path, .. }) if path == "PACKAGES")
    );
    let object = RObject::from_parts(
        RValue::Character(vec![RStr::new(
            b"x",
            REncoding::Native,
            NativeEncodingSource::Unknown,
        )]),
        Attributes::new(vec![Attribute::new(
            Symbol::new("dim"),
            RObject::from_parts(
                RValue::Integer(vec![Some(1), Some(1)]),
                Attributes::default(),
            ),
        )]),
    );
    assert!(
        matches!(PackagesMatrix::from_object(&object), Err(ViewError::Missing { path, .. }) if path == "PACKAGES.attributes.dimnames")
    );
    let mut object = valid();
    replace_dimnames(&mut object, vec![names(&["row"])]);
    assert!(
        matches!(PackagesMatrix::from_object(&object), Err(ViewError::UnexpectedLength { path, .. }) if path == "PACKAGES.attributes.dimnames")
    );
    let object = matrix(
        vec![Some(i32::MAX), Some(i32::MAX)],
        vec![
            RObject::from_parts(RValue::Null, Attributes::default()),
            RObject::from_parts(RValue::Character(vec![]), Attributes::default()),
        ],
        vec![],
    );
    // i32::MAX * i32::MAX fits in a 64-bit usize (data-length mismatch)
    // but overflows a 32-bit usize (dimension-product overflow).
    let error = PackagesMatrix::from_object(&object).unwrap_err();
    if usize::BITS >= 64 {
        assert!(
            matches!(error, ViewError::UnexpectedLength { ref path, .. } if path == "PACKAGES")
        );
    } else {
        assert!(matches!(error, ViewError::InvalidDimensions { .. }));
    }
}

#[test]
fn packages_matrix_rejects_na_and_duplicate_column_names() {
    let mut object = matrix(
        vec![Some(1), Some(2)],
        vec![names(&["row"]), names(&["Package", "Package"])],
        vec![
            RStr::new(b"x", REncoding::Native, NativeEncodingSource::Unknown),
            RStr::new(b"y", REncoding::Native, NativeEncodingSource::Unknown),
        ],
    );
    assert!(
        matches!(PackagesMatrix::from_object(&object), Err(ViewError::DuplicateName { path, .. }) if path == "PACKAGES.attributes.dimnames[1][1]")
    );
    replace_dimnames(
        &mut object,
        vec![
            names(&["row"]),
            RObject::from_parts(
                RValue::Character(vec![
                    RStr::Na,
                    RStr::new(b"Version", REncoding::Native, NativeEncodingSource::Unknown),
                ]),
                Attributes::default(),
            ),
        ],
    );
    assert!(
        matches!(PackagesMatrix::from_object(&object), Err(ViewError::UnexpectedType { path, .. }) if path == "PACKAGES.attributes.dimnames[1][0]")
    );
}
