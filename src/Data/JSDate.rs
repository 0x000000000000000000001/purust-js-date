use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

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

// `Foreign` cannot name this crate's `JSDate` (dependency cycle), so the
// `readDate` tag check is exposed here instead of through `tagOf`.
pub fn Data_JSDate_foreignIsDate(value: crate::UnknownType) -> bool {
    matches!(value.resolve(), crate::Value::Class(native) if native.downcast_ref::<Rc<JSDate>>().is_some())
}

pub fn Data_JSDate_jsdate(record: crate::UnknownType) -> Rc<JSDate> {
    // Date.UTC takes a zero-based month; the shared helper is one-based.
    let fields = record_fields(&record);
    jsdate_utc(&fields)
}

pub fn Data_JSDate_jsdateLocal(record: crate::UnknownType) -> crate::UnknownType {
    let fields = record_fields(&record);
    crate::Value::Func1(purust_core::Func1::Shared(Rc::new(move |_| {
        crate::Value::Class(Rc::new(jsdate_local(&fields)))
    })))
}

pub fn Data_JSDate_now() -> crate::UnknownType {
    crate::Value::Func1(purust_core::Func1::Static(|_| {
        let milliseconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as f64)
            .unwrap_or(0.0);
        crate::Value::Class(Rc::new(Data_JSDate_fromTime(milliseconds)))
    }))
}

pub fn Data_JSDate_parse(source: String) -> crate::UnknownType {
    crate::Value::Func1(purust_core::Func1::Shared(Rc::new(move |_| {
        crate::Value::Class(Rc::new(parse_date(&source)))
    })))
}

pub fn Data_JSDate_dateMethod() -> crate::UnknownType {
    crate::Value::Func2(purust_core::Func2::Shared(Rc::new(|method, date| {
        let name = method.unwrap_string();
        let date = date.unwrap_class::<Rc<JSDate>>().clone();
        method_value(&name, &date)
    })))
}

pub fn Data_JSDate_dateMethodEff() -> crate::UnknownType {
    crate::Value::Func2(purust_core::Func2::Shared(Rc::new(|method, date| {
        let name = method.unwrap_string();
        let date = date.unwrap_class::<Rc<JSDate>>().clone();
        crate::Value::Func1(purust_core::Func1::Shared(Rc::new(move |_| {
            method_value(&name, &date)
        })))
    })))
}

fn record_fields(record: &crate::UnknownType) -> [f64; 7] {
    [
        record.get_year().unwrap_number(),
        record.get_month().unwrap_number() + 1.0,
        record.get_day().unwrap_number(),
        record.get_hour().unwrap_number(),
        record.get_minute().unwrap_number(),
        record.get_second().unwrap_number(),
        record.get_millisecond().unwrap_number(),
    ]
}

fn integral(fields: &[f64; 7]) -> Option<[i64; 7]> {
    if fields.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let mut out = [0_i64; 7];
    for (index, value) in fields.iter().enumerate() {
        out[index] = value.trunc() as i64;
    }
    Some(out)
}

fn jsdate_utc(fields: &[f64; 7]) -> Rc<JSDate> {
    match integral(fields) {
        Some(f) => Data_JSDate_fromTime(Purs_Data_Date::purust_utc_milliseconds(
            f[0], f[1], f[2], f[3], f[4], f[5], f[6],
        )),
        None => Data_JSDate_fromTime(f64::NAN),
    }
}

fn jsdate_local(fields: &[f64; 7]) -> Rc<JSDate> {
    match integral(fields) {
        Some(f) => {
            let year = f[0];
            let (first_year, second_year) = if (0..100).contains(&year) {
                (year + 1900, Some(year))
            } else {
                (year, None)
            };
            let mut tm = local_tm(first_year, f[1], f[2], f[3], f[4], f[5]);
            if let Some(restored) = second_year {
                // Reproduce `new Date(...)` followed by `setFullYear(year)`.
                let normalized = local_fields(tm);
                tm = local_tm(restored, normalized[1], normalized[2], normalized[3],
                    normalized[4], normalized[5]);
            }
            Data_JSDate_fromTime(tm_to_milliseconds(tm) + f[6] as f64)
        }
        None => Data_JSDate_fromTime(f64::NAN),
    }
}

// Minimal `struct tm` shared by the C library's local-time entry points.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CTm {
    tm_sec: i32,
    tm_min: i32,
    tm_hour: i32,
    tm_mday: i32,
    tm_mon: i32,
    tm_year: i32,
    tm_wday: i32,
    tm_yday: i32,
    tm_isdst: i32,
    tm_gmtoff: i64,
    tm_zone: *const i8,
}

extern "C" {
    fn mktime(tm: *mut CTm) -> i64;
    fn localtime_r(time: *const i64, tm: *mut CTm) -> *mut CTm;
}

fn local_tm(year: i64, month: i64, day: i64, hour: i64, minute: i64, second: i64) -> CTm {
    let mut tm = CTm {
        tm_sec: second as i32,
        tm_min: minute as i32,
        tm_hour: hour as i32,
        tm_mday: day as i32,
        tm_mon: (month - 1) as i32,
        tm_year: (year - 1900) as i32,
        tm_isdst: -1,
        ..Default::default()
    };
    unsafe {
        mktime(&mut tm);
    }
    tm
}

fn local_fields(tm: CTm) -> [i64; 6] {
    [
        tm.tm_year as i64 + 1900,
        tm.tm_mon as i64 + 1,
        tm.tm_mday as i64,
        tm.tm_hour as i64,
        tm.tm_min as i64,
        tm.tm_sec as i64,
    ]
}

fn tm_to_milliseconds(mut tm: CTm) -> f64 {
    let seconds = unsafe { mktime(&mut tm) };
    seconds as f64 * 1000.0
}

fn local_tm_at(milliseconds: f64) -> Option<CTm> {
    if !milliseconds.is_finite() {
        return None;
    }
    let seconds = (milliseconds / 1000.0).floor() as i64;
    let mut tm = CTm::default();
    let result = unsafe { localtime_r(&seconds, &mut tm) };
    if result.is_null() { None } else { Some(tm) }
}

fn utc_parts(milliseconds: f64) -> Option<([i64; 3], [i64; 4])> {
    if !milliseconds.is_finite() {
        return None;
    }
    let ms = milliseconds.trunc() as i64;
    let days = ms.div_euclid(86_400_000);
    let time = ms.rem_euclid(86_400_000);
    let date = Purs_Data_Date::purust_date_from_days(days);
    Some((date, [time / 3_600_000, (time / 60_000) % 60, (time / 1000) % 60, time % 1000]))
}

fn weekday_name(weekday: i64) -> &'static str {
    ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
        .get(weekday.rem_euclid(7) as usize)
        .copied()
        .unwrap_or("Sun")
}

fn month_name(month: i64) -> &'static str {
    ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
        .get((month - 1).rem_euclid(12) as usize)
        .copied()
        .unwrap_or("Jan")
}

fn method_value(name: &str, date: &Rc<JSDate>) -> crate::UnknownType {
    let ms = date.milliseconds;
    match name {
        "getTime" => crate::mk_number(ms),
        "getUTCFullYear" => crate::mk_number(utc_part(ms, 0)),
        "getUTCMonth" => crate::mk_number(utc_part(ms, 1)),
        "getUTCDate" => crate::mk_number(utc_part(ms, 2)),
        "getUTCDay" => crate::mk_number(utc_weekday(ms)),
        "getUTCHours" => crate::mk_number(utc_part(ms, 3)),
        "getUTCMinutes" => crate::mk_number(utc_part(ms, 4)),
        "getUTCSeconds" => crate::mk_number(utc_part(ms, 5)),
        "getUTCMilliseconds" => crate::mk_number(utc_part(ms, 6)),
        "getFullYear" => crate::mk_number(local_number(ms, 0)),
        "getMonth" => crate::mk_number(local_number(ms, 1)),
        "getDate" => crate::mk_number(local_number(ms, 2)),
        "getDay" => crate::mk_number(local_number(ms, 3)),
        "getHours" => crate::mk_number(local_number(ms, 4)),
        "getMinutes" => crate::mk_number(local_number(ms, 5)),
        "getSeconds" => crate::mk_number(local_number(ms, 6)),
        "getMilliseconds" => crate::mk_number(local_number(ms, 7)),
        "getTimezoneOffset" => crate::mk_number(match local_tm_at(ms) {
            Some(tm) => -(tm.tm_gmtoff as f64) / 60.0,
            None => f64::NAN,
        }),
        "toISOString" => match utc_parts(ms) {
            Some((date, time)) => crate::Value::String(format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
                date[0], date[1], date[2], time[0], time[1], time[2], time[3]
            )),
            None => panic!("Invalid time value"),
        },
        "toUTCString" => match utc_parts(ms) {
            Some((date, time)) => {
                let weekday = (date_days(date) + 4).rem_euclid(7);
                crate::Value::String(format!(
                    "{}, {:02} {} {} {:02}:{:02}:{:02} GMT",
                    weekday_name(weekday), date[2], month_name(date[1]), date[0],
                    time[0], time[1], time[2]
                ))
            }
            None => crate::Value::String("Invalid Date".to_owned()),
        },
        "toDateString" => match local_tm_at(ms) {
            Some(tm) => crate::Value::String(format!(
                "{} {} {:02} {}",
                weekday_name(tm.tm_wday as i64),
                month_name(tm.tm_mon as i64 + 1),
                tm.tm_mday,
                tm.tm_year as i64 + 1900
            )),
            None => crate::Value::String("Invalid Date".to_owned()),
        },
        "toTimeString" => match local_tm_at(ms) {
            Some(tm) => crate::Value::String(format!(
                "{:02}:{:02}:{:02} {}",
                tm.tm_hour, tm.tm_min, tm.tm_sec, offset_text(tm)
            )),
            None => crate::Value::String("Invalid Date".to_owned()),
        },
        "toString" => match local_tm_at(ms) {
            Some(tm) => crate::Value::String(format!(
                "{} {} {:02} {} {:02}:{:02}:{:02} {}",
                weekday_name(tm.tm_wday as i64),
                month_name(tm.tm_mon as i64 + 1),
                tm.tm_mday,
                tm.tm_year as i64 + 1900,
                tm.tm_hour, tm.tm_min, tm.tm_sec,
                offset_text(tm)
            )),
            None => crate::Value::String("Invalid Date".to_owned()),
        },
        other => panic!("Data.JSDate: unsupported date method {}", other),
    }
}

fn offset_text(tm: CTm) -> String {
    let minutes = tm.tm_gmtoff / 60;
    let sign = if minutes < 0 { "-" } else { "+" };
    let zone = if tm.tm_zone.is_null() {
        String::new()
    } else {
        let bytes = unsafe { std::ffi::CStr::from_ptr(tm.tm_zone) };
        bytes.to_string_lossy().into_owned()
    };
    format!("GMT{}{:02}{:02} ({})", sign, minutes.abs() / 60, minutes.abs() % 60, zone)
}

fn date_days(date: [i64; 3]) -> i64 {
    Purs_Data_Date::date_days(date[0], date[1], date[2])
}

fn utc_part(ms: f64, index: usize) -> f64 {
    let Some((date, time)) = utc_parts(ms) else {
        return f64::NAN;
    };
    match index {
        0 => date[0] as f64,
        1 => (date[1] - 1) as f64,
        2 => date[2] as f64,
        3 => time[0] as f64,
        4 => time[1] as f64,
        5 => time[2] as f64,
        _ => time[3] as f64,
    }
}

fn utc_weekday(ms: f64) -> f64 {
    match utc_parts(ms) {
        Some((date, _)) => ((date_days(date) + 4).rem_euclid(7)) as f64,
        None => f64::NAN,
    }
}

fn local_number(ms: f64, index: usize) -> f64 {
    match local_tm_at(ms) {
        Some(tm) => match index {
            0 => (tm.tm_year as i64 + 1900) as f64,
            1 => tm.tm_mon as f64,
            2 => tm.tm_mday as f64,
            3 => tm.tm_wday as f64,
            4 => tm.tm_hour as f64,
            5 => tm.tm_min as f64,
            6 => tm.tm_sec as f64,
            _ => (ms - (ms / 1000.0).floor() * 1000.0).floor() as f64,
        },
        None => f64::NAN,
    }
}

// A compact subset of the ECMAScript Date.parse grammar: ISO 8601 date and
// date-time forms (with optional Z or numeric offset) and the RFC 2822 form
// produced by `Date.prototype.toUTCString`.
fn parse_date(source: &str) -> Rc<JSDate> {
    let text = source.trim();
    if let Some(fields) = parse_iso(text) {
        return jsdate_local_or_utc(fields.0, fields.1);
    }
    if let Some(fields) = parse_rfc2822(text) {
        return jsdate_utc(&fields);
    }
    Data_JSDate_fromTime(f64::NAN)
}

// (local fields, optional utc offset in minutes)
fn parse_iso(text: &str) -> Option<([f64; 7], Option<i64>)> {
    let bytes = text.as_bytes();
    let digits = |start: usize, count: usize| -> Option<i64> {
        if start + count > bytes.len() { return None; }
        let slice = &text[start..start + count];
        if !slice.bytes().all(|b| b.is_ascii_digit()) { return None; }
        slice.parse::<i64>().ok()
    };
    let year = digits(0, 4)?;
    let mut fields = [year as f64, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0];
    let mut offset = None;
    let mut index = 4;
    if index < bytes.len() && bytes[index] == b'-' {
        fields[1] = digits(index + 1, 2)? as f64;
        index += 3;
        if index < bytes.len() && bytes[index] == b'-' {
            fields[2] = digits(index + 1, 2)? as f64;
            index += 3;
        }
    }
    if index < bytes.len() && (bytes[index] == b'T' || bytes[index] == b' ') {
        fields[3] = digits(index + 1, 2)? as f64;
        if index + 3 <= bytes.len() && bytes.get(index + 3) == Some(&b':') {
            fields[4] = digits(index + 4, 2)? as f64;
            index += 6;
            if bytes.get(index) == Some(&b':') {
                fields[5] = digits(index + 1, 2)? as f64;
                index += 3;
                if bytes.get(index) == Some(&b'.') {
                    let millis = digits(index + 1, 3)? as f64;
                    fields[6] = millis;
                    index += 4;
                }
            }
        } else {
            index += 3;
        }
        match bytes.get(index) {
            Some(b'Z') => offset = Some(0),
            Some(b'+') | Some(b'-') => {
                let sign = if bytes[index] == b'+' { 1 } else { -1 };
                let hours = digits(index + 1, 2)?;
                let minutes = if bytes.get(index + 3) == Some(&b':') {
                    digits(index + 4, 2)?
                } else {
                    digits(index + 3, 2)?
                };
                offset = Some(sign * (hours * 60 + minutes));
                index += if bytes.get(index + 3) == Some(&b':') { 6 } else { 5 };
            }
            _ => {}
        }
    }
    if index != bytes.len() {
        return None;
    }
    Some((fields, offset))
}

fn parse_rfc2822(text: &str) -> Option<[f64; 7]> {
    let mut rest = text;
    if let Some(comma) = rest.find(',') {
        rest = rest[comma + 1..].trim_start();
    }
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    if tokens.len() < 4 {
        return None;
    }
    let day: f64 = tokens[0].parse().ok()?;
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|name| name.eq_ignore_ascii_case(tokens[1]))? as f64
        + 1.0;
    let year: f64 = tokens[2].parse().ok()?;
    let time: Vec<&str> = tokens[3].split(':').collect();
    if time.len() != 3 {
        return None;
    }
    let hour: f64 = time[0].parse().ok()?;
    let minute: f64 = time[1].parse().ok()?;
    let second: f64 = time[2].parse().ok()?;
    let offset = tokens.get(4).and_then(|token| parse_offset(token)).unwrap_or(0);
    // Shift the offset form into UTC fields.
    let total = hour * 60.0 + minute - offset as f64;
    let mut fields = [year, month, day, 0.0, 0.0, second, 0.0];
    fields[3] = (total / 60.0).floor();
    fields[4] = total.rem_euclid(60.0);
    Some(fields)
}

fn parse_offset(token: &str) -> Option<i64> {
    match token {
        "GMT" | "UTC" | "Z" => return Some(0),
        _ => {}
    }
    let bytes = token.as_bytes();
    if bytes.len() < 4 {
        return None;
    }
    let sign = match bytes[0] {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let hours: i64 = token[1..3].parse().ok()?;
    let minutes: i64 = token[3..5].parse().ok()?;
    Some(sign * (hours * 60 + minutes))
}

fn jsdate_local_or_utc(fields: [f64; 7], offset: Option<i64>) -> Rc<JSDate> {
    match integral(&fields) {
        Some(f) => match offset {
            Some(minutes) => Data_JSDate_fromTime(
                Purs_Data_Date::purust_utc_milliseconds(f[0], f[1], f[2], f[3], f[4], f[5], f[6])
                    - (minutes * 60_000) as f64,
            ),
            None => jsdate_local(&fields),
        },
        None => Data_JSDate_fromTime(f64::NAN),
    }
}
