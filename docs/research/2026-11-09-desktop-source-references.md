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
