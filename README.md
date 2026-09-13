# purust-js-date

PureScript and JavaScript sources copied unchanged from `js-date` 8.0.0.
The Rust FFI currently implements only TimeClip construction (`fromTime`,
`fromInstant`), validity and conversion to Instant. The native opaque handle
stores milliseconds, including an invalid-date NaN, not a `Data.Date.Date`.

Other foreign operations remain compiler fallbacks and must be guarded before
executing a suite. This is not a complete JavaScript Date implementation:
calendar construction, parsing, formatting, local time and clock effects are
not yet ported. No mutable Date operation is exposed by this library API.
