//! "Does not panic" properties over arbitrary input (A§9.3).

use oraculum_core::catalog::{Catalog, Source};
use oraculum_core::generator::Generator;
use oraculum_core::table::Table;
use oraculum_core::template::{Template, TemplateContext};
use proptest::prelude::*;

const VALID_TABLE: &str = "name = \"T\"\ndice = \"d6\"\n[[entries]]\nrange = [1, 3]\ntext = \"a {table:x}\"\n[[entries]]\nrange = [4, 6]\ntext = \"b {roll:1d4}\"\n";
const VALID_GEN: &str = "name = \"G\"\ntemplate = \"{s} {roll:d6}\"\n[[slots]]\nkey = \"s\"\ntable = \"t\"\ncount = \"1d3\"\nchance = 50\n";

/// Delete, duplicate or replace a random slice of `base`.
fn mutate(base: &'static str) -> impl Strategy<Value = String> {
    (0..base.len(), 0..8usize, ".{0,12}").prop_map(move |(at, cut, ins)| {
        let mut s: Vec<char> = base.chars().collect();
        let at = at.min(s.len());
        let end = (at + cut).min(s.len());
        s.splice(at..end, ins.chars());
        s.into_iter().collect()
    })
}

proptest! {
    #[test]
    fn template_parse_never_panics(s in ".{0,80}") {
        let _ = Template::parse(&s, TemplateContext::TableEntry);
        let _ = Template::parse(&s, TemplateContext::GeneratorTemplate);
    }

    #[test]
    fn table_from_toml_never_panics(s in ".{0,200}") {
        let _ = Table::from_toml(&s, "stem");
    }

    #[test]
    fn mutated_tables_never_panic(s in mutate(VALID_TABLE)) {
        let _ = Table::from_toml(&s, "stem");
    }

    #[test]
    fn generator_from_toml_never_panics(s in ".{0,200}") {
        let _ = Generator::from_toml(&s, "stem");
    }

    #[test]
    fn mutated_generators_never_panic(s in mutate(VALID_GEN)) {
        let _ = Generator::from_toml(&s, "stem");
    }

    #[test]
    fn catalog_build_never_panics_and_returns_warnings(
        t in mutate(VALID_TABLE),
        g in mutate(VALID_GEN),
        u in mutate(VALID_TABLE),
    ) {
        let builtin = [Source::table("b", "t", &t), Source::generator("b", "g", &g)];
        let user = [Source::table("u", "t", &u)];
        let (cat, _warnings) = Catalog::build(&builtin, &user);
        // Whatever survived must be usable.
        let mut roller = oraculum_core::rng::RngRoller::from_seed(1);
        for item in cat.items() {
            let _ = match item.kind {
                oraculum_core::catalog::ItemKind::Table => cat.roll_table(&item.id, &mut roller).map(|_| ()),
                oraculum_core::catalog::ItemKind::Generator => cat.generate(&item.id, &mut roller).map(|_| ()),
            };
        }
    }
}
