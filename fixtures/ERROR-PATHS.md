# Error paths

Which of S-0002's error classes (R2) each command and verb has, and the case that
covers each. `n/a` gives the reason a class doesn't apply. Every case under
`fixtures/cases/` is named here once, and a self-test (`cli.rs`,
`error_paths_name_every_case`) checks that both ways.

The classes:

- **args**: unknown flags, flags missing their value, missing or extra arguments.
- **context**: outside a workspace, or with no active change of the right kind.
- **order**: a phase out of order, or an item in the wrong state, under each
  `gates` setting where it applies.
- **unsafe**: a write outside the workspace, or through a symlink.
- **corrupt**: an unreadable `STATE.json`, `REPOS.json`, `.daiku/settings.json`,
  global `settings.json`, ticket front matter, or other file the command reads.
- **other**: the command working, and its other outcomes.

## daiku (no command)

| Class | Cases |
| --- | --- |
| args | `no-arguments`, `unknown-command`, `unknown-command-help-flag` |
| context | n/a: dispatch only |
| order | n/a: dispatch only |
| unsafe | n/a: dispatch only |
| corrupt | n/a: dispatch only |

## daiku init

| Class | Cases |
| --- | --- |
| args | `init-two-dirs`, `init-unknown-flag` |
| context | n/a: makes a workspace; init-existing and init-dir-argument cover where it runs |
| order | n/a: no phase order: works in any phase |
| unsafe | `init-through-symlink` |
| corrupt | n/a: doesn't read STATE.json or settings; it writes them |
| other | `init-dir-argument`, `init-existing`, `init-force`, `init-fresh`, `init-git`, `init-help-flag` |

## daiku update

| Class | Cases |
| --- | --- |
| args | `update-extra-argument` |
| context | `update-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | `update-through-symlink` |
| corrupt | `update-corrupt-manifest` |
| other | `update-edited-file`, `update-help-flag`, `update-keeps-non-utf8-agents`, `update-unchanged` |

## daiku status

| Class | Cases |
| --- | --- |
| args | `status-extra-argument` |
| context | `status-no-change`, `status-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: writes nothing |
| corrupt | `status-corrupt-state` |
| other | `status-build-change`, `status-git-workspace`, `status-help-flag`, `status-plan-argument`, `status-plan-change`, `status-repo-explore-skipped`, `status-unknown-plan` |

## daiku settings

| Class | Cases |
| --- | --- |
| args | `settings-extra-argument` |
| context | `settings-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: writes nothing |
| corrupt | `settings-bad-gates-value`, `settings-bad-overview-value`, `settings-invalid-global`, `settings-invalid-project` |
| other | `settings-defaults`, `settings-global`, `settings-help-flag`, `settings-project`, `settings-project-overrides-global` |

## daiku switch

| Class | Cases |
| --- | --- |
| args | `switch-to-change-json-flag`, `switch-two-changes`, `switch-unknown-flag` |
| context | `switch-no-changes`, `switch-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | `switch-through-symlink` |
| corrupt | `switch-corrupt-state` |
| other | `switch-all`, `switch-closed`, `switch-help-flag`, `switch-list`, `switch-list-all-closed`, `switch-list-closed-build-keeps-loop`, `switch-list-hides-closed`, `switch-to-change`, `switch-unknown` |

## daiku source new

| Class | Cases |
| --- | --- |
| args | `source-new-extra-argument`, `source-new-flag-missing-value`, `source-new-missing-title`, `source-new-unknown-flag` |
| context | `source-new-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | `source-new-dir-outside-workspace`, `source-new-through-symlink` |
| corrupt | `source-new-corrupt-state` |
| other | `source-new`, `source-new-help-flag`, `source-new-repo` |

## daiku source list

| Class | Cases |
| --- | --- |
| args | `source-list-extra-argument` |
| context | `source-list-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: writes nothing |
| corrupt | `source-list-bad-front-matter`, `source-list-invalid-settings` |
| other | `source-list`, `source-list-default-verb`, `source-list-empty`, `source-list-help-flag`, `source-list-non-utf8` |

## daiku source show

| Class | Cases |
| --- | --- |
| args | `source-show-extra-arg`, `source-show-invalid-id`, `source-show-missing-id` |
| context | `source-show-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: writes nothing |
| corrupt | `source-show-bad-front-matter` |
| other | `source-show`, `source-show-help-flag`, `source-show-non-utf8`, `source-show-unknown` |

## daiku adr new

| Class | Cases |
| --- | --- |
| args | `adr-new-extra-argument`, `adr-new-missing-title`, `adr-new-unknown-flag` |
| context | `adr-new-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | `adr-new-dir-absolute`, `adr-new-dir-outside-workspace`, `adr-new-through-symlink` |
| corrupt | `adr-new-invalid-settings` |
| other | `adr-new`, `adr-new-file-missing`, `adr-new-file-non-utf8`, `adr-new-from-file`, `adr-new-help-flag`, `adr-new-title-only` |

## daiku adr list

| Class | Cases |
| --- | --- |
| args | `adr-list-extra-argument` |
| context | `adr-list-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: writes nothing |
| corrupt | `adr-list-invalid-settings` |
| other | `adr-list`, `adr-list-default-verb`, `adr-list-empty`, `adr-list-help-flag`, `adr-list-non-utf8` |

## daiku adr show

| Class | Cases |
| --- | --- |
| args | `adr-show-extra-arg`, `adr-show-invalid-number`, `adr-show-missing-number` |
| context | `adr-show-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: writes nothing |
| corrupt | `adr-show-bad-file` |
| other | `adr-show`, `adr-show-help-flag`, `adr-show-non-utf8`, `adr-show-unknown` |

## daiku build new

| Class | Cases |
| --- | --- |
| args | `build-new-extra-argument`, `build-new-missing-id`, `build-new-unknown-flag` |
| context | `build-new-outside-workspace`, `build-new-unregistered-repo` |
| order | `build-new-draft-ticket` |
| unsafe | `build-new-through-symlink` |
| corrupt | `build-new-corrupt-front-matter`, `build-new-corrupt-repos`, `build-new-corrupt-state`, `build-new-invalid-global` |
| other | `build-new`, `build-new-again`, `build-new-epic`, `build-new-git`, `build-new-help-flag`, `build-new-inbox-ticket`, `build-new-pulled-ticket`, `build-new-ready-ticket`, `build-new-unknown-ticket` |

## daiku change new

| Class | Cases |
| --- | --- |
| args | `change-new-flag-missing-value`, `change-new-missing-title`, `change-new-unknown-flag`, `change-new-unquoted-title` |
| context | `change-new-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | `change-new-through-symlink` |
| corrupt | `change-new-corrupt-state`, `change-new-invalid-global` |
| other | `change-new`, `change-new-body-rev`, `change-new-git`, `change-new-help-flag`, `change-new-registered-repo`, `change-new-repo-path`, `change-new-second`, `change-new-two-repos-with-path`, `change-new-unknown-repo-name` |

## daiku change close

| Class | Cases |
| --- | --- |
| args | `change-close-missing-change`, `change-close-missing-reason`, `change-close-unknown-flag` |
| context | `change-close-outside-workspace` |
| order | `change-close-already-closed` |
| unsafe | `change-close-through-symlink` |
| corrupt | `change-close-corrupt-state` |
| other | `change-close`, `change-close-build`, `change-close-git`, `change-close-help-flag`, `change-close-unknown` |

## daiku gate

| Class | Cases |
| --- | --- |
| args | `gate-bad-status-for-phase`, `gate-cant-skip`, `gate-extra-argument`, `gate-missing-status`, `gate-reason-missing-value`, `gate-skip-needs-reason`, `gate-unknown-flag`, `gate-unknown-phase`, `gate-unknown-phase-forced`, `gate-unknown-status` |
| context | `gate-closed-change`, `gate-no-active-change`, `gate-outside-workspace` |
| order | `gate-architecture-no-diagrams`, `gate-build-out-of-order`, `gate-build-repo-held`, `gate-build-repo-held-forced`, `gate-build-repo-held-prompt`, `gate-build-skip-out-of-order`, `gate-out-of-order-auto`, `gate-out-of-order-blocked`, `gate-out-of-order-forced`, `gate-out-of-order-global-overridden`, `gate-out-of-order-global-prompt`, `gate-out-of-order-prompt`, `gate-publish-complete-unpublished`, `gate-spec-artifact-forced`, `gate-spec-artifact-problems`, `gate-violations-listed-together` |
| unsafe | `gate-log-through-symlink`, `gate-through-symlink` |
| corrupt | `gate-corrupt-state`, `gate-out-of-order-invalid-global`, `gate-out-of-order-invalid-project` |
| other | `gate-breakdown-approved`, `gate-build-implement-complete`, `gate-build-plan-phase`, `gate-build-ship-complete`, `gate-build-ticket-approved`, `gate-git-breakdown`, `gate-git-build-implement`, `gate-git-intake`, `gate-git-refused-commits-nothing`, `gate-git-ship-complete`, `gate-git-spec`, `gate-help-flag`, `gate-intake-approved`, `gate-intake-await-approval`, `gate-publish-in-progress`, `gate-refresh-keeps-non-utf8`, `gate-research-skipped`, `gate-spec-approved` |

## daiku reopen

| Class | Cases |
| --- | --- |
| args | `reopen-extra-argument`, `reopen-missing-phase`, `reopen-missing-reason`, `reopen-reason-missing-value`, `reopen-unknown-flag`, `reopen-unknown-phase` |
| context | `reopen-closed-change`, `reopen-no-active-change` |
| order | `reopen-not-started` |
| unsafe | `reopen-through-symlink` |
| corrupt | `reopen-corrupt-state` |
| other | `reopen-build-design`, `reopen-build-implement`, `reopen-git`, `reopen-help-flag`, `reopen-intake-keeps-skips`, `reopen-spec` |

## daiku tickets new

| Class | Cases |
| --- | --- |
| args | `tickets-new-flag-missing-value`, `tickets-new-json-flag`, `tickets-new-inbox-missing-repo`, `tickets-new-missing-title`, `tickets-new-task-missing-repo`, `tickets-new-unknown-flag`, `tickets-new-unknown-type`, `tickets-new-unquoted-title` |
| context | `tickets-new-no-change-needs-repo`, `tickets-new-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | `tickets-new-through-symlink` |
| corrupt | `tickets-new-corrupt-state`, `tickets-new-unreadable-ticket` |
| other | `tickets-new-epic`, `tickets-new-feature-parent`, `tickets-new-help-flag`, `tickets-new-in-build-goes-to-inbox`, `tickets-new-inbox`, `tickets-new-inbox-not-task`, `tickets-new-story-full`, `tickets-new-task`, `tickets-new-unknown-repo`, `tickets-new-unknown-requirement` |

## daiku tickets pull

| Class | Cases |
| --- | --- |
| args | `tickets-pull-missing-argument`, `tickets-pull-missing-file` |
| context | `tickets-pull-in-build`, `tickets-pull-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | `tickets-pull-through-symlink` |
| corrupt | `tickets-pull-bad-front-matter`, `tickets-pull-corrupt-state` |
| other | `tickets-pull`, `tickets-pull-help-flag`, `tickets-pull-no-external`, `tickets-pull-twice` |

## daiku tickets check

| Class | Cases |
| --- | --- |
| args | `tickets-check-extra-argument` |
| context | `tickets-check-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: writes nothing |
| corrupt | `tickets-check-corrupt-front-matter` |
| other | `tickets-check-clean`, `tickets-check-drafts`, `tickets-check-go-published-html`, `tickets-check-help-flag`, `tickets-check-no-tickets`, `tickets-check-problems` |

## daiku tickets status

| Class | Cases |
| --- | --- |
| args | `tickets-status-extra-argument` |
| context | `tickets-status-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: writes nothing |
| corrupt | `tickets-status-corrupt-front-matter` |
| other | `tickets-status`, `tickets-status-drifted`, `tickets-status-help-flag`, `tickets-status-mixed` |

## daiku tickets order

| Class | Cases |
| --- | --- |
| args | `tickets-order-extra-argument` |
| context | `tickets-order-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: writes nothing |
| corrupt | `tickets-order-corrupt-front-matter` |
| other | `tickets-order`, `tickets-order-cycle`, `tickets-order-help-flag` |

## daiku tickets ready

| Class | Cases |
| --- | --- |
| args | `tickets-ready-ids-and-all`, `tickets-ready-no-ids` |
| context | `tickets-ready-in-build`, `tickets-ready-outside-workspace` |
| order | `tickets-ready-already-ready`, `tickets-ready-before-breakdown`, `tickets-ready-before-breakdown-forced` |
| unsafe | `tickets-ready-through-symlink` |
| corrupt | `tickets-ready-corrupt-front-matter`, `tickets-ready-corrupt-state` |
| other | `tickets-ready-all`, `tickets-ready-help-flag`, `tickets-ready-html-title`, `tickets-ready-one`, `tickets-ready-placeholder`, `tickets-ready-unknown` |

## daiku tickets mark-published

| Class | Cases |
| --- | --- |
| args | `tickets-mark-published-bad-external`, `tickets-mark-published-missing-external`, `tickets-mark-published-unknown-flag` |
| context | `tickets-mark-published-outside-workspace` |
| order | `tickets-mark-published-before-publish`, `tickets-mark-published-draft`, `tickets-mark-published-draft-forced`, `tickets-mark-published-parent-unpublished` |
| unsafe | `tickets-mark-published-through-symlink` |
| corrupt | `tickets-mark-published-corrupt-front-matter`, `tickets-mark-published-corrupt-state` |
| other | `tickets-mark-published`, `tickets-mark-published-again`, `tickets-mark-published-help-flag`, `tickets-mark-published-html-same-id`, `tickets-mark-published-unknown` |

## daiku tickets unfreeze

| Class | Cases |
| --- | --- |
| args | `tickets-unfreeze-missing-reason` |
| context | `tickets-unfreeze-outside-workspace` |
| order | `tickets-unfreeze-not-frozen` |
| unsafe | `tickets-unfreeze-through-symlink` |
| corrupt | `tickets-unfreeze-corrupt-front-matter` |
| other | `tickets-unfreeze`, `tickets-unfreeze-help-flag`, `tickets-unfreeze-unknown` |

## daiku overview

| Class | Cases |
| --- | --- |
| args | `overview-check-and-stdout`, `overview-two-changes`, `overview-unknown-flag` |
| context | `overview-check-no-active-change`, `overview-in-build`, `overview-no-active-change` |
| order | n/a: no phase order: works in any phase |
| unsafe | `overview-through-symlink` |
| corrupt | `overview-corrupt-state` |
| other | `overview`, `overview-check-current`, `overview-check-non-utf8`, `overview-check-stale`, `overview-help-flag`, `overview-keeps-non-utf8`, `overview-manual-setting`, `overview-named-change`, `overview-stdout`, `overview-stdout-non-utf8`, `overview-unknown-change`, `overview-up-to-date` |

## daiku idea new

| Class | Cases |
| --- | --- |
| args | `idea-new-missing-title`, `idea-new-unknown-flag`, `idea-new-unquoted-title` |
| context | `idea-new-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | `idea-new-through-symlink` |
| corrupt | `idea-new-corrupt-summary` |
| other | `idea-new`, `idea-new-first`, `idea-new-help-flag` |

## daiku idea list

| Class | Cases |
| --- | --- |
| args | `idea-list-extra-argument` |
| context | `idea-list-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: writes nothing |
| corrupt | `idea-list-corrupt-summary` |
| other | `idea-list`, `idea-list-empty`, `idea-list-help-flag`, `idea-list-non-utf8` |

## daiku idea show

| Class | Cases |
| --- | --- |
| args | `idea-show-invalid-id`, `idea-show-missing-id` |
| context | `idea-show-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: writes nothing |
| corrupt | `idea-show-corrupt-summary` |
| other | `idea-show`, `idea-show-help-flag`, `idea-show-non-utf8`, `idea-show-unknown` |

## daiku idea park

| Class | Cases |
| --- | --- |
| args | `idea-park-missing-reason`, `idea-park-unknown-flag` |
| context | `idea-park-outside-workspace` |
| order | `idea-park-already-parked` |
| unsafe | `idea-park-through-symlink` |
| corrupt | `idea-park-corrupt-summary` |
| other | `idea-park`, `idea-park-help-flag`, `idea-park-non-utf8`, `idea-park-unknown` |

## daiku idea drop

| Class | Cases |
| --- | --- |
| args | `idea-drop-extra-argument` |
| context | `idea-drop-outside-workspace` |
| order | `idea-drop-already-dropped` |
| unsafe | `idea-drop-through-symlink` |
| corrupt | `idea-drop-corrupt-summary` |
| other | `idea-drop`, `idea-drop-help-flag` |

## daiku idea reopen

| Class | Cases |
| --- | --- |
| args | `idea-reopen-missing-reason` |
| context | `idea-reopen-outside-workspace` |
| order | `idea-reopen-open` |
| unsafe | `idea-reopen-through-symlink` |
| corrupt | `idea-reopen-corrupt-summary` |
| other | `idea-reopen-dropped`, `idea-reopen-help-flag`, `idea-reopen-parked` |

## daiku idea promote

| Class | Cases |
| --- | --- |
| args | `idea-promote-missing-title`, `idea-promote-unknown-flag` |
| context | `idea-promote-outside-workspace` |
| order | `idea-promote-parked` |
| unsafe | `idea-promote-through-symlink` |
| corrupt | `idea-promote-corrupt-state`, `idea-promote-corrupt-summary` |
| other | `idea-promote`, `idea-promote-help-flag`, `idea-promote-non-utf8`, `idea-promote-unknown`, `idea-promote-with-repo` |

## daiku references check

| Class | Cases |
| --- | --- |
| args | `references-check-extra-argument` |
| context | `references-check-no-folder` |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: writes nothing |
| corrupt | n/a: reads only references/, never state or settings |
| other | `references-check-clean`, `references-check-findings`, `references-check-help-flag`, `references-check-no-index` |

## daiku repo add

| Class | Cases |
| --- | --- |
| args | `repo-add-json-flag`, `repo-add-missing-remote`, `repo-add-unknown-flag` |
| context | `repo-add-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | `repo-add-through-symlink` |
| corrupt | `repo-add-corrupt-repos` |
| other | `repo-add`, `repo-add-dash-remote`, `repo-add-existing-name`, `repo-add-help-flag`, `repo-add-home-path`, `repo-add-invalid-name`, `repo-add-path-branch`, `repo-add-relative-path`, `repo-add-remote-credentials`, `repo-add-remote-helper` |

## daiku repo list

| Class | Cases |
| --- | --- |
| args | `repo-list-extra-argument` |
| context | `repo-list-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: writes nothing |
| corrupt | `repo-list-corrupt-paths`, `repo-list-corrupt-repos`, `repo-list-invalid-repo-name` |
| other | `repo-list`, `repo-list-empty`, `repo-list-help-flag`, `repo-list-masks-credentials`, `repo-list-no-path` |

## daiku repo path

| Class | Cases |
| --- | --- |
| args | `repo-path-missing-path` |
| context | `repo-path-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | `repo-path-through-symlink` |
| corrupt | `repo-path-corrupt-paths` |
| other | `repo-path`, `repo-path-help-flag`, `repo-path-unknown` |

## daiku repo doctor

| Class | Cases |
| --- | --- |
| args | `repo-doctor-into-missing-value`, `repo-doctor-unknown-flag` |
| context | `repo-doctor-missing-on-disk`, `repo-doctor-no-local-path`, `repo-doctor-not-a-checkout`, `repo-doctor-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: --clone's one workspace write is .daiku/paths.json, the writer covered by repo-path-through-symlink |
| corrupt | `repo-doctor-clone-invalid-repo-name`, `repo-doctor-corrupt-repos` |
| other | `repo-doctor-clone`, `repo-doctor-clone-no-into`, `repo-doctor-clone-nothing-missing`, `repo-doctor-empty`, `repo-doctor-help-flag`, `repo-doctor-mismatch-credentials`, `repo-doctor-no-origin` |

## daiku env

| Class | Cases |
| --- | --- |
| args | `env-extra-argument` |
| context | `env-no-change`, `env-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: writes nothing |
| corrupt | `env-corrupt-repos`, `env-corrupt-state`, `env-invalid-repo-name` |
| other | `env-build-change`, `env-help-flag`, `env-plan-without-repos` |

## daiku commit

| Class | Cases |
| --- | --- |
| args | n/a: every argument goes to git commit unchanged |
| context | `commit-no-active-build`, `commit-no-change`, `commit-no-local-path`, `commit-not-a-checkout`, `commit-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: daiku writes nothing; git does the commit |
| corrupt | `commit-corrupt-state` |
| other | `commit-help-flag`, `commit-nothing-staged`, `commit-staged`, `commit-staged-two-paragraphs` |

## daiku pr

| Class | Cases |
| --- | --- |
| args | n/a: only a leading title is read; the rest is ignored |
| context | `pr-no-active-build`, `pr-not-a-checkout`, `pr-outside-workspace` |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: daiku writes nothing; gh opens the pull request |
| corrupt | `pr-corrupt-state` |
| other | `pr-gh-missing`, `pr-help-flag`, `pr-title-gh-missing` |

## daiku check

| Class | Cases |
| --- | --- |
| args | `check-extra-argument` |
| context | n/a: needs no workspace or change |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: writes nothing |
| corrupt | n/a: reads no workspace state |
| other | `check`, `check-help-flag` |

## daiku version

| Class | Cases |
| --- | --- |
| args | `version-extra-argument` |
| context | n/a: needs no workspace or change |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: writes nothing |
| corrupt | n/a: reads no workspace state |
| other | `version`, `version-flag`, `version-help-flag` |

## daiku help

| Class | Cases |
| --- | --- |
| args | `help-unknown-command`, `help-unknown-verb` |
| context | n/a: needs no workspace or change |
| order | n/a: no phase order: works in any phase |
| unsafe | n/a: writes nothing |
| corrupt | n/a: reads no workspace state |
| other | `help`, `help-adr`, `help-adr-list`, `help-adr-new`, `help-adr-show`, `help-build`, `help-build-new`, `help-change`, `help-change-close`, `help-change-new`, `help-check`, `help-commit`, `help-env`, `help-flag`, `help-flag-after-arguments`, `help-flag-after-double-dash`, `help-gate`, `help-help`, `help-help-flag`, `help-idea`, `help-idea-drop`, `help-idea-list`, `help-idea-new`, `help-idea-park`, `help-idea-promote`, `help-idea-reopen`, `help-idea-show`, `help-init`, `help-overview`, `help-pr`, `help-references`, `help-references-check`, `help-reopen`, `help-repo`, `help-repo-add`, `help-repo-doctor`, `help-repo-list`, `help-repo-path`, `help-settings`, `help-short-flag`, `help-source`, `help-source-list`, `help-source-new`, `help-source-show`, `help-status`, `help-switch`, `help-tickets`, `help-tickets-check`, `help-tickets-mark-published`, `help-tickets-new`, `help-tickets-order`, `help-tickets-pull`, `help-tickets-ready`, `help-tickets-status`, `help-tickets-unfreeze`, `help-update`, `help-verb-flag`, `help-version` |

## daiku adr (no verb, or an unknown one)

| Class | Cases |
| --- | --- |
| args | `adr-unknown-verb` |
| context | n/a: a command with verbs: the verbs' rows cover this class |
| order | n/a: a command with verbs: the verbs' rows cover this class |
| unsafe | n/a: a command with verbs: the verbs' rows cover this class |
| corrupt | n/a: a command with verbs: the verbs' rows cover this class |
| other | `adr-help-flag` |

## daiku build (no verb, or an unknown one)

| Class | Cases |
| --- | --- |
| args | `build-no-verb`, `build-unknown-verb` |
| context | n/a: a command with verbs: the verbs' rows cover this class |
| order | n/a: a command with verbs: the verbs' rows cover this class |
| unsafe | n/a: a command with verbs: the verbs' rows cover this class |
| corrupt | n/a: a command with verbs: the verbs' rows cover this class |
| other | `build-help-flag` |

## daiku change (no verb, or an unknown one)

| Class | Cases |
| --- | --- |
| args | `change-no-verb`, `change-unknown-verb` |
| context | n/a: a command with verbs: the verbs' rows cover this class |
| order | n/a: a command with verbs: the verbs' rows cover this class |
| unsafe | n/a: a command with verbs: the verbs' rows cover this class |
| corrupt | n/a: a command with verbs: the verbs' rows cover this class |
| other | `change-help-flag` |

## daiku idea (no verb, or an unknown one)

| Class | Cases |
| --- | --- |
| args | `idea-no-verb`, `idea-unknown-verb` |
| context | n/a: a command with verbs: the verbs' rows cover this class |
| order | n/a: a command with verbs: the verbs' rows cover this class |
| unsafe | n/a: a command with verbs: the verbs' rows cover this class |
| corrupt | n/a: a command with verbs: the verbs' rows cover this class |
| other | `idea-help-flag` |

## daiku references (no verb, or an unknown one)

| Class | Cases |
| --- | --- |
| args | `references-no-verb` |
| context | n/a: a command with verbs: the verbs' rows cover this class |
| order | n/a: a command with verbs: the verbs' rows cover this class |
| unsafe | n/a: a command with verbs: the verbs' rows cover this class |
| corrupt | n/a: a command with verbs: the verbs' rows cover this class |
| other | `references-help-flag` |

## daiku repo (no verb, or an unknown one)

| Class | Cases |
| --- | --- |
| args | `repo-no-verb`, `repo-unknown-verb` |
| context | n/a: a command with verbs: the verbs' rows cover this class |
| order | n/a: a command with verbs: the verbs' rows cover this class |
| unsafe | n/a: a command with verbs: the verbs' rows cover this class |
| corrupt | n/a: a command with verbs: the verbs' rows cover this class |
| other | `repo-help-flag` |

## daiku source (no verb, or an unknown one)

| Class | Cases |
| --- | --- |
| args | `source-unknown-verb` |
| context | n/a: a command with verbs: the verbs' rows cover this class |
| order | n/a: a command with verbs: the verbs' rows cover this class |
| unsafe | n/a: a command with verbs: the verbs' rows cover this class |
| corrupt | n/a: a command with verbs: the verbs' rows cover this class |
| other | `source-help-flag` |

## daiku tickets (no verb, or an unknown one)

| Class | Cases |
| --- | --- |
| args | `tickets-no-verb`, `tickets-unknown-verb` |
| context | n/a: a command with verbs: the verbs' rows cover this class |
| order | n/a: a command with verbs: the verbs' rows cover this class |
| unsafe | n/a: a command with verbs: the verbs' rows cover this class |
| corrupt | n/a: a command with verbs: the verbs' rows cover this class |
| other | `tickets-help-flag` |
