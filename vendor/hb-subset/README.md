# Local HarfBuzz subset binding

Rust wrapper: mkwebfont_hb-subset 0.5.0 (MIT).
Native source: HarfBuzz 14.6.0 official release archive, preserving its COPYING.
Source: https://github.com/harfbuzz/harfbuzz/releases/tag/14.6.0
Archive SHA256: d07a007327277708a2a73ae437887cdbaf282937f6d03ca5467723e9099af586

The wrapper's stable C ABI is unchanged. The native source is updated from 9.0.0 because CFF output from the old backend failed a second subsetting operation. C++17 is selected through cc for Unix and MSVC portability. No system library is required.
