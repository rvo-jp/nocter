use super::lower_module;

#[test]
fn only_reached_retirement_contributes_executable_destructor_bodies() {
    let program = lower_module(
        "func run(): i32",
        concat!(
            "struct Live { value: i32 }\n",
            "impl Live { drop { self.value += 1 } }\n",
            "struct Unused { value: i32 }\n",
            "impl Unused { drop { unused_helper() } }\n",
            "func unused_helper(): void {}\n",
            "struct Carrier { child: Live }\n",
            "func run(): i32 {\n",
            "    let value = Carrier { child: Live { value: 42 } }\n",
            "    drop value\n",
            "    42\n",
            "}",
        ),
    );
    let (&receiver, &body) = program.destructors().first_key_value().unwrap();
    assert_eq!(program.destructors().len(), 1);
    assert_eq!(program.bodies().len(), 2);
    assert_ne!(program.root(), body);
    assert_eq!(
        program.bodies()[&body].signature().explicit()[0].data,
        receiver
    );
    assert_eq!(program.cleanup().len(), 2);
    assert!(program.cleanup().contains(&receiver));
}
