use super::*;
use std::str;

#[test]
fn scope_empty() {
    let mut buffer = vec![];

    {
        let _scope = Scope::new(Writer::from(&mut buffer));
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(
        buffer, "",
        "A newly created Scope should not emit anything."
    );
}

#[test]
fn structure_empty() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let _structure = scope.structure(Visibility::Private, "Test").unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, "struct Test {\n}\n\n");
}

#[test]
fn structure_empty_public() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let _structure = scope.structure(Visibility::Public, "Test").unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, "pub struct Test {\n}\n\n");
}

#[test]
fn structure_repr_c() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let _structure = scope
            .line("#[repr(C)]")
            .unwrap()
            .structure(Visibility::Private, "Test")
            .unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("structure_repr_c.expected"));
}

#[test]
fn structure_repr_c_public() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let _structure = scope
            .line("#[repr(C)]")
            .unwrap()
            .structure(Visibility::Public, "Test")
            .unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("structure_repr_c_public.expected"));
}

#[test]
fn structure_multiline_annotation() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let _structure = scope
            .line("#[repr(C)]")
            .unwrap()
            .line("/// Second line")
            .unwrap()
            .line("/// Third line")
            .unwrap()
            .structure(Visibility::Private, "Test")
            .unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(
        buffer,
        include_str!("structure_multiline_annotation.expected")
    );
}

#[test]
fn structure_single_field() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));

        let mut structure = scope.structure(Visibility::Private, "Test").unwrap();

        structure.field("field1", "u32").unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("structure_single_field.expected"));
}

#[test]
fn structure_multiple_fields() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));

        let mut structure = scope.structure(Visibility::Private, "Test").unwrap();

        structure
            .field("field1", "u32")
            .unwrap()
            .field("field2", "Option<(bool, f32, String, i128)>")
            .unwrap()
            .field("field3", format_args!("[{}; {}]", "u8", 32))
            .unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("structure_multiple_fields.expected"));
}

#[test]
fn structure_annotate_fields() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));

        let mut structure = scope.structure(Visibility::Private, "Test").unwrap();

        structure
            .line("// 0x0(0x4)")
            .unwrap()
            .field("field1", "u32")
            .unwrap()
            .line("#[test(attr)]")
            .unwrap()
            .field("field2", "Option<(bool, f32, String, i128)>")
            .unwrap()
            .line("// Multi-")
            .unwrap()
            .line("// Line")
            .unwrap()
            .field("field3", format_args!("[{}; {}]", "u8", 32))
            .unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("structure_annotate_fields.expected"));
}

#[test]
fn enum_single_variant() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let mut e = scope.enumeration(Visibility::Private, "TestEnum").unwrap();
        e.variant("TestVariant1").unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("enum_single_variant.expected"));
}

#[test]
fn enum_single_variant_public() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let mut e = scope.enumeration(Visibility::Public, "TestEnum").unwrap();
        e.variant("TestVariant1").unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("enum_single_variant_public.expected"));
}

#[test]
fn enum_single_variant_annotate() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let mut e = scope
            .line("#[repr(u8)]")
            .unwrap()
            .enumeration(Visibility::Private, "TestEnum")
            .unwrap();

        e.line("// Test annotation for first variant.")
            .unwrap()
            .line("#[test(attr)]")
            .unwrap()
            .variant("TestVariant1")
            .unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(
        buffer,
        include_str!("enum_single_variant_annotate.expected")
    );
}

#[test]
fn enum_multiple_variants() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let mut e = scope.enumeration(Visibility::Private, "TestEnum").unwrap();

        e.variant("TestVariant1")
            .unwrap()
            .variant("TestVariant2")
            .unwrap()
            .variant("TestVariant3")
            .unwrap()
            .variant("TestVariant4")
            .unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("enum_multiple_variants.expected"));
}

#[test]
fn impl_empty() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let _imp = scope.imp("Struct").unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("impl_empty.expected"));
}

#[test]
fn impl_trait_empty() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let _imp = scope.imp_trait("Trait", "Struct").unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("impl_trait_empty.expected"));
}

#[test]
fn impl_line() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let _imp = scope
            .imp("Struct")
            .unwrap()
            .line("// I'm a line inside of an `impl` block.")
            .unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("impl_line.expected"));
}

#[test]
fn impl_fn() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let args = [["arg1", "typ1"], ["arg2", "typ2"], ["arg3", "typ3"]];
        let ret = "impl Iterator<Item = u8>";
        scope
            .imp("Struct")
            .unwrap()
            .function_args_ret(Nil, "test", &args, ret)
            .unwrap()
            .line("// Function implementation.")
            .unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("impl_fn.expected"));
}

#[test]
fn impl_method_no_args_no_ret() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        scope
            .imp("Struct")
            .unwrap()
            .function_args(Nil, "test", args!("&mut self"))
            .unwrap()
            .line("// Function implementation.")
            .unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("impl_method_no_args_no_ret.expected"));
}

#[test]
fn impl_method_args_no_ret() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let args = args!(
            "&mut self",
            [["arg1", "typ1"], ["arg2", "typ2"], ["arg3", "typ3"]].iter()
        );
        scope
            .imp("Struct")
            .unwrap()
            .function_args(Nil, "test", args)
            .unwrap()
            .line("// Function implementation.")
            .unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("impl_method_args_no_ret.expected"));
}

#[test]
fn impl_method_args_ret() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let args = args!(
            "&mut self",
            [["arg1", "typ1"], ["arg2", "typ2"], ["arg3", "typ3"]].iter()
        );
        let ret = "impl Iterator<Item = u8>";
        scope
            .imp("Struct")
            .unwrap()
            .function_args_ret(Nil, "test", args, ret)
            .unwrap()
            .line("// Function implementation.")
            .unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("impl_method_args_ret.expected"));
}

#[test]
fn fn_no_args_no_ret() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let _function = scope.function(Visibility::Private, "test").unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("fn_no_args_no_ret.expected"));
}

#[test]
fn fn_no_args_no_ret_public() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let _function = scope.function("pub ", "test").unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("fn_no_args_no_ret_public.expected"));
}

#[test]
fn fn_args_no_ret() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let args = [["arg1", "typ1"], ["arg2", "typ2"], ["arg3", "typ3"]];
        let _function = scope
            .function_args(Nil, "test", &args)
            .unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("fn_args_no_ret.expected"));
}

#[test]
fn fn_args_ret() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        let args = [["arg1", "typ1"], ["arg2", "typ2"], ["arg3", "typ3"]];
        let ret = "impl Iterator<Item = u8>";
        let _function = scope
            .function_args_ret(Nil, "test", &args, ret)
            .unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("fn_args_ret.expected"));
}

#[test]
fn fn_single_line_body() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        scope
            .function(Visibility::Private, "test")
            .unwrap()
            .line("let x = 2 - 3 + 5 - 7 + 11 - 13;")
            .unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("fn_single_line_body.expected"));
}

#[test]
fn fn_if_block() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        scope
            .function(Visibility::Private, "test")
            .unwrap()
            .if_block("if let Some(function) = FUNCTION")
            .unwrap()
            .line("// If block.")
            .unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("fn_if_block.expected"));
}

#[test]
fn fn_else_block() {
    let mut buffer = vec![];

    {
        let mut scope = Scope::new(Writer::from(&mut buffer));
        scope
            .function(Visibility::Private, "test")
            .unwrap()
            .if_block("if let Some(function) = FUNCTION")
            .unwrap()
            .line("// If block.")
            .unwrap()
            .else_block("else")
            .unwrap()
            .line("// Else block.")
            .unwrap();
    }

    let buffer = str::from_utf8(&buffer).unwrap();

    assert_eq!(buffer, include_str!("fn_else_block.expected"));
}
