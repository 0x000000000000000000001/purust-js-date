use std::rc::Rc;

// An opaque native Date, distinct from both PureScript Date and Instant.
// This API exposes no setters; handles can safely share immutable milliseconds.
pub struct JSDate {
    milliseconds: f64,
}

const JS_DATE_LIMIT: f64 = 8_640_000_000_000_000.0;

pub fn Data_JSDate_fromTime(time: f64) -> Rc<JSDate> {
    let milliseconds = if !time.is_finite() || time.abs() > JS_DATE_LIMIT {
        f64::NAN
    } else {
        // ECMAScript TimeClip truncates toward zero and canonicalizes -0.
        let integer = time.trunc();
        if integer == 0.0 { 0.0 } else { integer }
    };
    Rc::new(JSDate { milliseconds })
}

pub fn Data_JSDate_fromInstant(time: f64) -> Rc<JSDate> {
    Data_JSDate_fromTime(time)
}

pub fn Data_JSDate_isValid(date: Rc<JSDate>) -> bool {
    date.milliseconds.is_finite()
}

pub fn Data_JSDate_toInstantImpl(
    just: purust_core::Func1<crate::UnknownType, Rc<Purs_Data_Maybe::Maybe>>,
    nothing: Rc<Purs_Data_Maybe::Maybe>,
    date: Rc<JSDate>,
) -> Rc<Purs_Data_Maybe::Maybe> {
    if date.milliseconds.is_nan() {
        nothing
    } else {
        just(crate::mk_number(date.milliseconds))
    }
}

#[cfg(test)]
mod js_date_tests {
    use super::*;

    #[test]
    fn time_clip_matches_js_date_boundaries() {
        for (input, expected) in [(0.0, 0.0), (-0.0, 0.0), (1.9, 1.0),
            (-1.9, -1.0), (-0.5, 0.0), (JS_DATE_LIMIT, JS_DATE_LIMIT),
            (-JS_DATE_LIMIT, -JS_DATE_LIMIT)] {
            let date = Data_JSDate_fromTime(input);
            assert!(Data_JSDate_isValid(date.clone()));
            assert_eq!(date.milliseconds.to_bits(), expected.to_bits());
            assert_eq!(Data_JSDate_fromInstant(input).milliseconds.to_bits(), expected.to_bits());
        }
        for input in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY,
            JS_DATE_LIMIT + 1.0, -JS_DATE_LIMIT - 1.0] {
            let date = Data_JSDate_fromTime(input);
            assert!(!Data_JSDate_isValid(date.clone()));
            assert!(date.milliseconds.is_nan());
        }
    }

    #[test]
    fn conversion_preserves_callbacks_and_invalid_identity() {
        let nothing = Purs_Data_Maybe::Data_Maybe_Nothing();
        let just = purust_core::Func1::Static(Purs_Data_Maybe::Data_Maybe_Just);
        let value = Data_JSDate_toInstantImpl(just.clone(), nothing.clone(), Data_JSDate_fromTime(1234.9));
        match value.as_ref() {
            Purs_Data_Maybe::Maybe::Just(number) => assert_eq!(number.unwrap_number(), 1234.0),
            _ => panic!("expected Just"),
        }
        let invalid = Data_JSDate_toInstantImpl(
            purust_core::Func1::Static(|_| panic!("invalid date must not call Just")),
            nothing.clone(), Data_JSDate_fromTime(f64::NAN));
        assert!(Rc::ptr_eq(&nothing, &invalid));
        // Exercise the actual generated PureScript conversion as well.
        match Data_JSDate_toInstant(Data_JSDate_fromTime(-1234.9)).as_ref() {
            Purs_Data_Maybe::Maybe::Just(number) => assert_eq!(number.unwrap_number(), -1234.0),
            _ => panic!("expected Just from generated toInstant"),
        }
    }
}
