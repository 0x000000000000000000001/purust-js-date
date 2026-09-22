pub fn Test_Main_myDate() -> crate::UnknownType {
    // Any valid native date satisfies the test, which only checks that
    // `readDate` accepts it.
    crate::Value::Class(std::rc::Rc::new(Purs_Data_JSDate::Data_JSDate_fromTime(0.0)))
}
