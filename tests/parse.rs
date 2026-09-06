use cpan_distribution_meta::{Error, ReleaseStatus, SpecVersion, parse, parse_json, parse_yaml};

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[test]
fn spec2_json() {
    let meta = parse(&fixture("spec2.json")).unwrap();

    assert_eq!(meta.name, "Module-Build");
    assert_eq!(meta.version, "0.4229");
    assert_eq!(meta.spec_version(), &SpecVersion::V2);
    assert_eq!(
        meta.abstract_.as_deref(),
        Some("Build and install Perl modules")
    );
    assert_eq!(meta.author, ["Ken Williams <kwilliams@cpan.org>"]);
    assert_eq!(meta.license, ["perl_5"]);
    assert!(meta.dynamic_config);
    assert_eq!(meta.release_status, ReleaseStatus::Stable);
    assert_eq!(meta.meta_spec.version, "2");
    assert_eq!(meta.keywords, ["builder", "installer"]);

    // prereqs across phases and relationships
    assert_eq!(meta.prereqs.configure.requires["Module::Build"], "0.42");
    assert_eq!(meta.prereqs.build.requires["ExtUtils::Install"], "0");
    assert_eq!(meta.prereqs.test.requires["Test::More"], "0.49");
    assert_eq!(meta.prereqs.runtime.requires["perl"], "5.006");
    assert_eq!(meta.prereqs.runtime.recommends["Archive::Tar"], "1.00");

    // dynamic accessors
    assert_eq!(
        meta.prereqs
            .phase("runtime")
            .unwrap()
            .relation("requires")
            .unwrap()["Cwd"],
        "0"
    );

    // no_index
    assert_eq!(meta.no_index.directory, ["t", "inc"]);
    assert_eq!(meta.no_index.package, ["DB"]);

    // provides
    let p = &meta.provides["Module::Build"];
    assert_eq!(p.file, "lib/Module/Build.pm");
    assert_eq!(p.version.as_deref(), Some("0.42"));

    // optional_features
    let feat = &meta.optional_features["domination"];
    assert_eq!(feat.description.as_deref(), Some("Take over the world"));
    assert_eq!(feat.prereqs.develop.requires["Genius::Evil"], "1.234");
    assert_eq!(feat.prereqs.runtime.requires["Machine::Weather"], "2.0");

    // resources
    let res = &meta.resources;
    assert_eq!(res.license, ["http://dev.perl.org/licenses/"]);
    assert_eq!(
        res.bugtracker.as_ref().unwrap().web.as_deref(),
        Some("https://github.com/Perl-Toolchain-Gang/Module-Build/issues")
    );
    let repo = res.repository.as_ref().unwrap();
    assert_eq!(repo.type_.as_deref(), Some("git"));
    assert_eq!(
        repo.url.as_deref(),
        Some("https://github.com/Perl-Toolchain-Gang/Module-Build.git")
    );
    assert_eq!(
        res.custom.get("x_MailingList").and_then(|v| v.as_str()),
        Some("mailto:module-build@perl.org")
    );

    // top-level custom key
    assert_eq!(
        meta.custom
            .get("x_serialization_backend")
            .and_then(|v| v.as_str()),
        Some("JSON::PP version 4.00")
    );
}

#[test]
fn spec2_yaml() {
    let meta = parse(&fixture("spec2.yml")).unwrap();
    assert_eq!(meta.name, "Module-Build");
    assert_eq!(meta.version, "0.4229");
    assert_eq!(meta.spec_version(), &SpecVersion::V2);
    assert_eq!(meta.license, ["perl_5"]);
    assert_eq!(meta.prereqs.runtime.requires["perl"], "5.006");
    assert_eq!(meta.prereqs.runtime.recommends["Archive::Tar"], "1.00");
    assert_eq!(
        meta.resources.repository.as_ref().unwrap().url.as_deref(),
        Some("https://github.com/Perl-Toolchain-Gang/Module-Build.git")
    );
}

#[test]
fn spec1_4_yaml_is_upgraded() {
    let meta = parse(&fixture("spec1_4.yml")).unwrap();

    assert_eq!(meta.name, "Foo-Bar");
    assert_eq!(meta.spec_version(), &SpecVersion::V1_4);
    assert!(meta.spec_version().is_legacy());

    // license string -> normalised v2 token list
    assert_eq!(meta.license, ["perl_5"]);

    // meta-spec is reported as the normalised version, url retained
    assert_eq!(meta.meta_spec.version, "2");
    assert_eq!(
        meta.meta_spec.url.as_deref(),
        Some("http://module-build.sourceforge.net/META-spec-v1.4.html")
    );

    // flat prereq fields folded into phases
    assert_eq!(meta.prereqs.runtime.requires["perl"], "5.006");
    assert_eq!(meta.prereqs.runtime.requires["Scalar::Util"], "1.10");
    assert_eq!(meta.prereqs.runtime.recommends["Data::Dumper"], "0");
    assert_eq!(meta.prereqs.runtime.conflicts["Foo::Bar"], "0");
    assert_eq!(meta.prereqs.build.requires["Test::More"], "0.47");
    assert_eq!(
        meta.prereqs.configure.requires["ExtUtils::MakeMaker"],
        "6.42"
    );

    // developer version -> testing (has underscore in a provides version, but
    // the dist version itself is stable)
    assert_eq!(meta.release_status, ReleaseStatus::Stable);

    // provides preserved, including underscore version
    assert_eq!(
        meta.provides["Foo::Bar::Baz"].version.as_deref(),
        Some("1.23_01")
    );

    // legacy optional_features (flat requires) upgraded
    let feat = &meta.optional_features["sql_support"];
    assert_eq!(feat.description.as_deref(), Some("Provides SQL storage"));
    assert_eq!(feat.prereqs.runtime.requires["DBI"], "1.50");

    // string resources -> structured
    let res = &meta.resources;
    assert_eq!(res.homepage.as_deref(), Some("http://example.com/foo-bar/"));
    assert_eq!(res.license, ["http://dev.perl.org/licenses/"]);
    assert_eq!(
        res.bugtracker.as_ref().unwrap().web.as_deref(),
        Some("http://rt.cpan.org/NoAuth/Bugs.html?Dist=Foo-Bar")
    );
    let repo = res.repository.as_ref().unwrap();
    assert_eq!(
        repo.url.as_deref(),
        Some("git://github.com/example/foo-bar.git")
    );
    assert_eq!(repo.type_.as_deref(), Some("git"));

    // obsolete key dropped
    assert!(!meta.custom.contains_key("distribution_type"));
}

#[test]
fn spec1_2_yaml_is_upgraded() {
    let meta = parse(&fixture("spec1_2.yml")).unwrap();

    assert_eq!(meta.name, "Acme-Old");
    assert_eq!(meta.spec_version(), &SpecVersion::V1_2);

    // bare "gpl" is ambiguous -> open_source
    assert_eq!(meta.license, ["open_source"]);

    assert_eq!(meta.prereqs.runtime.requires["perl"], "5.005");
    assert_eq!(meta.prereqs.build.requires["Test::More"], "0");
    assert_eq!(meta.prereqs.runtime.recommends["Time::HiRes"], "0");

    // pre-1.3 `dir` key normalised to `directory`
    assert_eq!(meta.no_index.directory, ["t", "inc"]);
    assert_eq!(meta.no_index.package, ["Acme::Old::Private"]);

    // capitalised `Repository` is a custom resource key in spec 1.2
    assert_eq!(
        meta.resources.homepage.as_deref(),
        Some("http://example.com/acme-old")
    );
    assert!(meta.resources.repository.is_none());
    assert_eq!(
        meta.resources
            .custom
            .get("Repository")
            .and_then(|v| v.as_str()),
        Some("http://svn.example.com/acme-old/trunk")
    );
}

#[test]
fn spec1_0_yaml_has_no_meta_spec() {
    let meta = parse(&fixture("spec1_0.yml")).unwrap();

    // no meta-spec field, no v2 markers -> assumed 1.0
    assert_eq!(meta.spec_version(), &SpecVersion::V1_0);
    assert_eq!(meta.name, "Ancient-Dist");
    assert_eq!(meta.version, "1.00");
    assert_eq!(meta.license, ["perl_5"]);
    assert_eq!(meta.prereqs.runtime.requires["perl"], "5.00503");
    assert_eq!(meta.prereqs.runtime.requires["Test::Harness"], "0");

    // empty build_requires produces no build phase
    assert!(meta.prereqs.build.is_empty());

    // required-by-later-specs fields are simply absent
    assert!(meta.abstract_.is_none());
    assert!(meta.author.is_empty());
    assert!(meta.generated_by.is_some());

    // defaults applied
    assert!(meta.dynamic_config);
    assert_eq!(meta.release_status, ReleaseStatus::Stable);
}

#[test]
fn format_autodetection() {
    // JSON content parses even via the generic entry point
    let json = r#"{ "name": "X", "version": "1" }"#;
    assert_eq!(parse(json).unwrap().name, "X");

    // YAML content parses via the generic entry point
    let yaml = "name: Y\nversion: '2'\n";
    assert_eq!(parse(yaml).unwrap().name, "Y");

    // explicit parsers
    assert_eq!(parse_json(json).unwrap().version, "1");
    assert_eq!(parse_yaml(yaml).unwrap().version, "2");
}

#[test]
fn release_status_derived_from_trial_version() {
    let yaml = "name: Trial-Dist\nversion: '1.00-TRIAL'\n";
    let meta = parse(yaml).unwrap();
    assert_eq!(meta.release_status, ReleaseStatus::Testing);

    let yaml = "name: Dev-Dist\nversion: '1.00_01'\n";
    let meta = parse(yaml).unwrap();
    assert_eq!(meta.release_status, ReleaseStatus::Testing);
}

#[test]
fn errors() {
    // not a mapping
    assert!(matches!(
        parse("[1, 2, 3]").unwrap_err(),
        Error::NotAMapping
    ));

    // missing required `name`
    assert!(matches!(
        parse("version: '1.0'").unwrap_err(),
        Error::MissingField("name")
    ));

    // empty input
    assert!(matches!(parse("   ").unwrap_err(), Error::UnknownFormat));

    // malformed
    assert!(matches!(
        parse_json("{ not json").unwrap_err(),
        Error::Json(_)
    ));
}

#[test]
fn roundtrips_to_json() {
    let meta = parse(&fixture("spec1_4.yml")).unwrap();
    let json = serde_json::to_string_pretty(&meta).unwrap();
    // re-parsing the serialised v2 form yields an equivalent structure
    let again = parse(&json).unwrap();
    assert_eq!(again.name, meta.name);
    assert_eq!(again.prereqs, meta.prereqs);
    assert_eq!(again.license, meta.license);
    assert_eq!(again.resources, meta.resources);
}
