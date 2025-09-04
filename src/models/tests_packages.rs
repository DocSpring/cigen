use crate::models::Job;

#[test]
fn parse_packages_shorthand_string() {
    let yaml = r#"
image: cimg/node:18
packages: node
"#;
    let job: Job = serde_yaml::from_str(yaml).unwrap();
    let pkgs = job.packages.unwrap();
    assert_eq!(pkgs.len(), 1);
    match &pkgs[0] {
        crate::models::job::PackageSpec::Simple(s) => assert_eq!(s, "node"),
        _ => panic!("expected simple package spec"),
    }
}

#[test]
fn parse_packages_object_with_path() {
    let yaml = r#"
image: cimg/node:18
packages:
  name: node
  path: docs
"#;
    let job: Job = serde_yaml::from_str(yaml).unwrap();
    let pkgs = job.packages.unwrap();
    assert_eq!(pkgs.len(), 1);
    match &pkgs[0] {
        crate::models::job::PackageSpec::WithPath { name, path } => {
            assert_eq!(name, "node");
            assert_eq!(path, "docs");
        }
        _ => panic!("expected with-path package spec"),
    }
}

