pub mod git;
pub mod template;

pub use git::GitAtom;
pub use template::{
    TemplateAtom, TemplateContext, TemplateEngine, TemplateInfo, TemplateSource, UndefinedBehavior,
    ValidationResult,
};
