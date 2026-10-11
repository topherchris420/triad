# REA capability audit

Vers3Dynamics — Christopher Woodyard. Audit date: 2026-10-11.

## Actual MCP observation

The documented MCP operation `binary_session` was invoked with `{}` and returned successfully. REA is registered, running, and accessible in this Codex session. This is a capability inspection, **not binary analysis**. No target was opened; `open` was false, `analysis_provider_binding` was null, and analysis activity was `not_observed`. No CAPSENSE archive, object, firmware, or executable was analyzed.

The live server identified `rea-agents` 6.4.0, server `rea` 6.4.0, and 170 MCP tools. Its installed package root was `C:\Users\chris\AppData\Roaming\npm\node_modules\rea-agents`, with command `scripts\rea.mjs`. The negotiated protocol was 2025-06-18. The server reported `codex-mcp-client` 0.159.2 and workflow version 37. The compact returned facts are preserved in [rea-session-evidence.json](../results/rea-session-evidence.json). A successfully returned status report establishes communication with the server; it does not establish an operational decompiler.

| Provider | Observed status | Returned reason and limits |
|---|---|---|
| Ghidra | Unavailable | `GHIDRA_INSTALL_DIR` was unset. Installation, headless executable, version, native decompiler, and Java checks failed. Diagnostics reported `win32`, `arm64`; the architecture check returned `unsupported_host`. Windows native controls require an x64 host. |
| Hopper | Unavailable | `unsupported_host`: Hopper integration is not supported on `win32`. |
| IDA Pro MCP adapter | Unavailable | `not_configured`: `REA_IDA_MCP_CONFIG` was unset; no live IDA connection was probed. |

These are observations about the connected process and its configuration. They do not prove that no Ghidra or Java installation exists anywhere on this computer. No broad filesystem inventory or installation was performed. All native provider target-support statuses were unknown because there was no active target. No provider became bound, and no native analysis ran.

## Documented setup and native requirements

The [REA README](https://github.com/morluto/rea/blob/main/README.md) describes MCP registration through `rea setup`, reviewed configuration changes, matching workflow instructions, and restarting the client. Merely installing a skill does not register MCP or install an engine. The [installation guide](https://github.com/morluto/rea/blob/main/docs/installation.md) documents provider configuration and scoped diagnostics. This audit did not change registrations or install providers.

The [Windows Ghidra P0 guide](https://github.com/morluto/rea/blob/main/docs/windows-ghidra-p0.md) limits that workflow to Windows 10+ **x64**, fixed local NTFS volumes, an operator-installed Ghidra 12.1.x release, a full compatible 64-bit JDK, and native non-managed x86 or x86-64 PE applications or DLLs. It supplies 25 read-only operations; other architectures/formats and GUI/database mutation are outside this Windows boundary. Its Python-free Java adapter does not need PyGhidra. This documented support does not authorize bypassing the observed ARM64 host restriction.

The [Ghidra provider contract](https://github.com/morluto/rea/blob/main/docs/ghidra-provider.md) documents Linux/macOS validation with host-native x86-64/AArch64 ELF or Mach-O, plus a cross-format verification matrix including AArch64 ELF, x86-64 PE, and x86-64 Mach-O. Separate [DOS](https://github.com/morluto/rea/blob/main/docs/ghidra-dos.md) and [MIPS](https://github.com/morluto/rea/blob/main/docs/ghidra-mips.md) guides describe bounded additional workflows. These are documented provider capabilities, not capabilities exercised by TRIAD or guarantees for every Ghidra loader. The [IDA provider guide](https://github.com/morluto/rea/blob/main/docs/ida-provider.md) requires a configured upstream IDA MCP registration. Its availability remains untested here.

REA's target schema can describe PE, ELF, Mach-O, DOS and packaged artifacts, and architectures including x86, x86-64, ARM, ARM64, and MIPS. Schema enumeration alone is not proof that the current provider/host can analyze all those combinations. In particular, **support for an Infineon ARM static `.a` archive has not been established**. Archive member inspection, object import, deep native analysis, and executing a commercial sensor algorithm are different operations.

## Evidence REA can return when a supported authorized target exists

The [MCP contracts](https://github.com/morluto/rea/blob/main/docs/mcp-contracts.md) define canonical evidence records with artifact SHA-256 and identity, provider/version, analysis profile, operation parameters, normalized and raw results, source locations, evidence links, limitations, and confidence/authority labels. Function tools can return analyzed functions, instructions, pseudocode, callers/callees, cross-references, control flow and bounded high-p-code data flow. Retained evidence can be inspected or exported. The status report itself is a configuration/result record, not a target-specific decompilation evidence record.

According to the Ghidra contract, pseudocode is decompiler output rather than original source. Cross-references and calls cover resolved analyzed edges; targetless indirect control flow remains unknown. Strings cover Ghidra-defined string data, so an empty string search does not establish absence from all bytes. P-code dossiers have explicit bounds and truncation reporting. Static evidence does not establish runtime behavior, persistent state, or algorithm correctness without additional corroboration.

Confidence values such as `observed`, `derived`, and `inferred` classify the evidential relationship; they are **not calibrated probabilities of reverse-engineering correctness**. No numeric decompilation-accuracy claim is made. Installed tool exposure, provider readiness, target admission, completed analysis, and evidential confidence must be recorded separately.

## Licensing and TRIAD scope

Infineon's restrictive license is reviewed separately in [commercial-reference-analysis.md](commercial-reference-analysis.md). Public download availability is not taken as permission to reverse engineer its precompiled algorithms. No restricted implementation is copied into TRIAD, and no commercial library is opened through REA. Any later binary investigation requires a suitable target, applicable permission, and a supported provider/host. Firmware analysis and commercial execution are unnecessary for Milestones 1 and 2.

REA is therefore available as an MCP service but **not ready for deep native analysis in the observed environment**. TRIAD's independent FEM and baseline localization experiments proceed without REA. Hidden commercial algorithm internals remain unknown, and no TRIAD/commercial accuracy comparison or commercial defect finding follows from this audit.
