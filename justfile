alias f := fmt
alias ff := fmt_force

# rustfmt.toml uses unstable options, so formatting needs nightly.
fmt:
  cargo +nightly fmt
  cargo fix
  cargo clippy --fix

fmt_force:
  cargo +nightly fmt
  cargo fix --allow-dirty
  cargo clippy --fix --allow-dirty
