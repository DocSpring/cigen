//! Display implementations for models

use super::job::{CacheRestore, Job, Step};

impl Job {
    pub fn pretty_print(&self) {
        println!("Job {{");
        println!("    image: {:?},", self.image);

        if let Some(architectures) = &self.architectures {
            println!("    architectures: [");
            for arch in architectures {
                println!("        {arch:?},");
            }
            println!("    ],");
        }

        if let Some(resource_class) = &self.resource_class {
            println!("    resource_class: {resource_class:?},");
        }

        if let Some(source_files) = &self.source_files {
            println!("    source_files: {source_files:?},");
        }

        if let Some(parallelism) = &self.parallelism {
            println!("    parallelism: {parallelism},");
        }

        if let Some(cache) = &self.cache {
            println!("    cache: {{");
            for (name, def) in cache {
                println!("        {name:?}: CacheDefinition {{");
                println!("            paths: {:?},", def.paths);
                println!("            restore: {},", def.restore);
                println!("        }},");
            }
            println!("    }},");
        }

        if let Some(restore_cache) = &self.restore_cache {
            println!("    restore_cache: [");
            for cache in restore_cache {
                match cache {
                    CacheRestore::Simple(name) => {
                        println!("        {name:?},");
                    }
                    CacheRestore::Complex { name, dependency } => {
                        println!("        {{ name: {name:?}, dependency: {dependency:?} }},");
                    }
                }
            }
            println!("    ],");
        }

        if let Some(services) = &self.services {
            println!("    services: {services:?},");
        }

        if let Some(steps) = &self.steps {
            println!("    steps: [");
            for step in steps {
                match step {
                    Step::Command(cmd) => {
                        println!("        {cmd:?},");
                    }
                    Step::Complex {
                        name,
                        run,
                        store_test_results,
                        store_artifacts,
                    } => {
                        println!("        {{");
                        if let Some(name) = name {
                            println!("            name: {name:?},");
                        }
                        if let Some(run) = run {
                            println!("            run: {run:?},");
                        }
                        if let Some(store_test_results) = store_test_results {
                            println!(
                                "            store_test_results: {{ path: {:?} }},",
                                store_test_results.path
                            );
                        }
                        if let Some(store_artifacts) = store_artifacts {
                            println!(
                                "            store_artifacts: {{ path: {:?} }},",
                                store_artifacts.path
                            );
                        }
                        println!("        }},");
                    }
                }
            }
            println!("    ],");
        }

        println!("}}");
    }
}
