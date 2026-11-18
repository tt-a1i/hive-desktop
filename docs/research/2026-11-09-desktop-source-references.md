# Desktop source references — 2026-11-09

The user explicitly permits referring to and reusing suitable open-source implementation, as well as visual reference. Compare source contracts with Hive before adopting; retain upstream notices when code is actually copied. This inspection imported no upstream source/assets and added no dependency.

## Beluga

Pinned `cab72cb7a85efef601651c516d4ddae46fde516f`, MIT metadata on GitHub.

Read [agent-setup.ts](https://github.com/kayraucklnc/beluga/blob/cab72cb7a85efef601651c516d4ddae46fde516f/packages/mac/src/onboarding/agent-setup.ts) and [Onboarding.tsx](https://github.com/kayraucklnc/beluga/blob/cab72cb7a85efef601651c516d4ddae46fde516f/packages/mac/src/onboarding/Onboarding.tsx).

Useful implementation boundaries: shared agent rows for onboarding/settings; separate ready/signed-out/missing/no-endpoint states and repair actions; unknown checks remain unknown; recheck only while an Agent can become ready. Its preferred-agent fallback is a product policy to evaluate separately, not an automatic fit for Hive's explicit selected backend/model. Hive currently has executable version checks, account metadata and real open errors; they do not yet establish the same complete readiness surface.

## T3 Code

Pinned `6647b0a761ff7a1287047a9bb5fd5b63772f456b`, [MIT license](https://github.com/pingdotgg/t3code/blob/6647b0a761ff7a1287047a9bb5fd5b63772f456b/LICENSE).

Read [ComposerFooterControls.tsx](https://github.com/pingdotgg/t3code/blob/6647b0a761ff7a1287047a9bb5fd5b63772f456b/apps/web/src/components/chat/ComposerFooterControls.tsx) and [CompactComposerControlsMenu.tsx](https://github.com/pingdotgg/t3code/blob/6647b0a761ff7a1287047a9bb5fd5b63772f456b/apps/web/src/components/chat/CompactComposerControlsMenu.tsx).

Useful boundaries: footer controls and overflow menu share the same actions; every trigger has an accessible label; hidden triggers close their portaled menu; focus returns intentionally after menu closure. React/Base UI components need GPUI counterparts, while pure state rules can be evaluated directly. The permission-mode UI in these files is specific to its product; Hive's confirmed built-in Pi policy remains the basis for Hive controls.

## Current application

Project-context and lead-selection UI reuse Hive handlers; see [native entry/repair](../validation/2026-11-09-project-entry.md). These are initial source-informed improvements, not claims of complete design/functional parity. Future source reuse should pin its exact file/revision and include notices for actual copied material.

2026-11-10: reread Beluga's same pinned `agent-setup.ts`. Added separate native welcome setup entries and existing version-probe summaries, retaining unknown/unsaved/changed states and explicit repair destinations; see [native setup verification](../validation/2026-11-10-welcome-agent-setup.md). No upstream code/assets, readiness policy or preferred-Agent fallback were copied. Full login/model readiness remains unimplemented.

## Apache Maka Rust preview

2026-11-18, reviewed `rust-preview` at `82b2628261463b04730df92097dccf37ca80ab84`. The desktop shell shuts down the old window engine before detaching and reconnecting. The host separately checks for accepted work before idle shutdown. Hive can apply those ownership boundaries to release dialog, image and conversation resources without changing the existing close-and-save behavior or the Hive collaboration ledger. [AppKit shell](https://github.com/apache/maka/blob/82b2628261463b04730df92097dccf37ca80ab84/products/maka/desktop-shell/src/appkit.rs) · [Host lifecycle](https://github.com/apache/maka/blob/82b2628261463b04730df92097dccf37ca80ab84/platform/host-runtime/src/application/host/lifecycle.rs)

## Desktop update workflows

- T3 Code `24e77473ecf41bc1f7142529c7911c1c1aae1d76` tracks availability, download and installation failures independently. Hive's About page needs those facts so a failed download or install retains a meaningful retry action. [Updater state](https://github.com/pingdotgg/t3code/blob/24e77473ecf41bc1f7142529c7911c1c1aae1d76/apps/desktop/src/updates/updateMachine.ts)
- Cindy waits for the old process to exit before replacing application files and distinguishes an ordinary install failure from a failed rollback. Hive needs to finish its own backend shutdown before the installer takes over and keep the old installation available until the new instance starts. [Installer](https://github.com/makecindy/cindy/blob/4fd670b72af9d060785e29c553a14b6794dfe8eb/apps/desktop/cindy-updater/src-tauri/src/installer.rs)
- Beluga checks compatibility and active work before installing. Hive's installation guard should consult its owned Agent turns, scripts and terminals. [Update rules](https://github.com/kayraucklnc/beluga/blob/cab72cb7a85efef601651c516d4ddae46fde516f/packages/mac/src-tauri/src/update_rules.rs)
- Maka records pending deployment state and can reconcile it after interruption. Hive's replacement and restart flow needs a durable receipt that can distinguish a staged update from a verified installation. [Deployment update](https://github.com/apache/maka/blob/82b2628261463b04730df92097dccf37ca80ab84/products/maka/cli/src/deployment/update.rs)

Hive will exercise the update workflow with an isolated development channel and disposable installations of two versions on each platform. No upstream source was copied for these changes.
