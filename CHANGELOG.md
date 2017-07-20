# wrapcenum-derive Changelog

This file describes the changes / additions / fixes between macro releases.

## 0.3.0 (released 2017-07-20)

### Changes

The `UnexpectedVariant` error kind is now expected to hold the value that caused the error.

## 0.2.0 (released 2017-06-08)

### Release Summary

The macro is now meant to be used with numerical constants instead of Rust enums. This was done for safety reasons; see [rust-lang/rust#36927](https://github.com/rust-lang/rust/issues/36927) for more information.

### Changes

* `has_count` attribute removed and replaced with `default`

## 0.1.0 (released 2017-05-17)

### Release Summary

Initial release providing the functionality necessary to wrap Rust `enum`-based C enum bindings.

```text
derive on Rust enum `Foo`
`Foo` wraps Rust enum `Bar`
`Bar` was auto-generated within bindings for C enum `Bar`
```
