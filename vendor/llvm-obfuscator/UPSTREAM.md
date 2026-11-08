# Bundled LLVM obfuscator

Source: https://github.com/eshard/obfuscator-llvm

Pinned revision: `ad26aa21b90cc5c5118d32114b674d1455f54168`.

The upstream implementation derives from Obfuscator-LLVM (HEIG-VD).
See `LICENSE-OBFUSCATOR.TXT` for the University of Illinois/NCSA license.
Keep this license with redistributed plugin binaries and source distributions.

cppm builds the plugin locally against the installed LLVM SDK; it does not
download or execute an unpinned prebuilt plugin. Sources are embedded in the
cppm executable, so installation and subsequent builds work offline.

Local changes:

- A CMake build that embeds the decoder without requiring Bash or xxd.
- Nonzero string masks and automatic storage for the encoding buffer.
- String encoding limited to local, non-TLS byte-array literals; shared ODR
  globals and aggregate constants are left alone to preserve C++ semantics.
- Decoder/init helpers use internal linkage; the decoder retains a noinline,
  optnone boundary through LTO. Imported debug information is discarded.
- Pass state is cleared when the module is entered again.
- Empty cloned debug compile-unit lists are removed for clean LTO bitcode,
  and the optional bogus pass checks an iterator before dereferencing it.

The managed preset selects `flattening,substitution,split-basic-blocks` in the
scalar optimizer's late extension point and `string-encryption` at optimizer
last. Other upstream extension-point environment variables are cleared for
the application build. The bogus-control-flow pass is bundled but not enabled
in this preset.

String masking is obfuscation, not cryptographic confidentiality. Not every
function or literal is eligible; exception-handling functions can be skipped.
The integration is validated with Clang/LLVM 23.1.4 on Linux. Other matching
LLVM 18–23 SDKs are accepted but must be verified with the target application.
