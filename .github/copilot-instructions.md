Read CONTRIBUTING.md at the root of this repository for general preferences.

Focus primarily on things like architecture, correctness, safety and proofs.

Generally ignore missing/extra imports (macros can make it confusing) and things you think will not compile if they are
going to be caught by the CI anyway, they will likely be either redundant or false-positives.

Breaking API changes are most often intentional, do not warn about them unless you have a strong reason for it. Most
crates are only used within the workspace, and even those that are published have no issues with bumping their version
accordingly when breaking changes do happen.
