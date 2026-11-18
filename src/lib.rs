//! swimflow — diagrams from a DOT-like text DSL.
//!
//! The crate is organized so that each diagram type's front end
//! (lexer/parser/AST) is independent of the shared layout and rendering
//! stages.

pub mod ast;
pub mod build;
pub mod cli;
pub mod core;
pub mod error;
pub mod lexer;
pub mod metrics;
pub mod parser;
pub mod plugin;
pub mod plugins;
pub mod render;
pub mod server;
pub mod text;

#[cfg(test)]
mod test_support;
