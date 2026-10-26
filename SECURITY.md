# Security policy

## Supported versions

Only the **latest release** on [GitHub Releases](https://github.com/guiselig/sdtally/releases) receives security fixes. Please check that a problem still happens on the latest release or on `main` before reporting it.

| Version | Supported |
|---|---|
| Latest release | Yes |
| Older releases | No |

## Reporting a vulnerability

Please **do not open a public issue** for a security problem.

Report it privately through GitHub:

1. Go to the repository's **Security** tab.
2. Click **Report a vulnerability**.
3. Describe the problem, the affected version, macOS version, and steps to reproduce. A proof of concept helps.

Do not include personal data from your Mac (real user names, home folder paths, file names) in the report unless it is needed to reproduce the issue.

Follow-up happens in the private advisory thread. Fixes ship as a new release, and the advisory is published once a fix is available.

## Scope

SDTally is read-only, and the app and the CLI do not use the network (only the optional updater does), so the most sensitive parts are the ones that run with elevated privileges or cross a trust boundary.

In scope:

- **The root helper** (`app/Sources/SDTallyHelper`, "Passwordless Root"): a LaunchDaemon registered by the app with `SMAppService`, started on demand through its Mach service `io.github.guiselig.sdtally.helper`.
- **The XPC interface** between the app and the helper (`app/Sources/SDTallyXPC/HelperXPC.swift`): four calls (`medir`, `parar`, `iniciarAuto`, `pararAuto`), no arguments from the client.
- **The code signing requirement** used on both sides: fixed identifier, `anchor apple generic`, and the same Team ID in `certificate leaf[subject.OU]` as the helper's own signature.
- **The binary copy check**: before running `sdtally`, the helper copies it from the app bundle into a new `0700` folder owned by root, refuses anything that is not a regular file (symlinks), sets it to `0500`, verifies its signature against the same-team requirement for the identifier `io.github.guiselig.sdtally.cli`, and executes that verified copy.
- **The optional updater** (`app/Sources/SDTallyUpdater`, off by default): a login item that asks `api.github.com` once a day for the latest release and, only after the user clicks Install, downloads the DMG from github.com, checks its SHA-256 against the digest of an immutable release, checks the new app's signature against the same identifier and Team ID, and replaces the SDTally app. A way to make it install something else, install without the click, or reach another host is in scope.
- **Running as root via the administrator password** (`osascript ... with administrator privileges`) in the app.
- **Anything that would make SDTally delete, move or modify user data**, or use the network. Both are design violations.
- **Redaction** (`--redact names`, MCP, exports): a case where the user name or a personal folder name leaks through redacted output.
- **The MCP server** (`sdtally --mcp`): for example, text from disk escaping the data boundary in a way that could instruct an AI agent.

Out of scope:

- The app not being notarized (documented; see below).
- Information that the user can already see without SDTally (the sizes of their own folders).
- Issues that require an attacker who is already root.

## How the helper limits what root can do

From the header of `app/Sources/SDTallyHelper/main.swift` and the code it describes:

- It accepts a connection only from `SDTally.app` signed by the **same team** as the helper (`setConnectionCodeSigningRequirement`).
- It accepts a connection only from a user in the **admin** group, the same requirement the password path already had.
- It runs only `sdtally --json` (full measurement) or `sdtally --auto` (live tree), with **fixed arguments** and a fixed environment (`SUDO_USER`, `SUDO_UID`, and `PATH=/usr/bin:/bin:/usr/sbin:/sbin`).
- It measures for the **user of the connection**, taken from the connection's audit token, not from anything the client says.
- It runs one full measurement at a time and one Automatic process at a time. Automatic lives only while the app's connection exists.
- Its only deletion is the temporary `0700` folder it created for the copy of `sdtally`.
- If the helper itself has no Team ID (ad hoc signature), it serves no one and exits.
- With no open connections and nothing running, it exits after about 10 seconds; launchd starts it again on the next connection.

## Known limitations

Stated as they are, from the helper's own header comment:

- **The trust anchor is "signed by the owner's team".** With an Apple Development certificate in the user's own keychain, a program running as that user that can use the private key could sign a fake `sdtally` and gain root through the helper. Release builds are produced by `app/montar.sh`, which signs with an Apple Development identity, not Developer ID.
- **The app bundle may be writable by the user.** The binary copy check closes the window between verifying and executing the binary, but it does not change who can produce a validly signed binary.
- What a proper distribution needs, and does not exist yet: Developer ID signing, the app installed in a root-owned location (`/Applications`), and a check that the bundle is not writable by the user.
- **Not notarized.** Users must use "Open Anyway" in System Settings > Privacy & Security on first launch.
- **Automatic with root** keeps a root process running while the app is open, and anyone using that login session can see the sizes of other users' folders. The app explains this before the option is enabled.

If you do not want these trade-offs, do not install Passwordless Root. Without it, a full measurement asks for the administrator password each time, and Automatic runs without root.
