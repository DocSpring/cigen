#!/usr/bin/env python3
import yaml
import json
import os
import sys

# Function to load YAML, handling potential errors
def load_yaml(filepath):
    try:
        with open(filepath, 'r') as f:
            return yaml.safe_load(f)
    except FileNotFoundError:
        print(f"Error: File not found at {filepath}", file=sys.stderr)
        sys.exit(1)
    except yaml.YAMLError as e:
        print(f"Error parsing YAML file {filepath}: {e}", file=sys.stderr)
        sys.exit(1)

def main():
    old_config_path = 'docspring/.circleci-orig/test_and_deploy_config.yml'
    new_config_path = 'docspring/.circleci/main.yml'

    old_config = load_yaml(old_config_path)
    new_config = load_yaml(new_config_path)

    old_jobs = set(old_config.get('jobs', {}).keys())
    new_jobs = set(new_config.get('jobs', {}).keys())

    # Mapping from Old Job Name -> New Job Name
    job_mapping = {
        'api_proxy': 'ci_api_proxy',
        'api_proxy_integration': 'ci_api_proxy_integration',
        'approve_build_app_image_eu': 'deploy_eu_approve_build',
        'approve_build_app_image_staging': 'deploy_staging_approve_build',
        'approve_build_app_image_us': 'deploy_us_approve_build',
        'bootsnap_precompile_amd64': 'ci_bootsnap',
        'build_app_image_amd64': 'build_build_app_image',
        'build_docker_images_amd64': 'build_build_docker_images',
        'build_enterprise_image_amd64': 'build_build_enterprise_image',
        'compile_client_webpack_production': 'build_compile_client_webpack_production',
        'compile_client_webpack_test': 'ci_compile_client_webpack_test',
        'compile_docs': 'ci_compile',
        'compile_rails_assets_production': 'build_compile_rails_assets_production',
        'compile_rails_assets_test': 'ci_compile_rails_assets_test',
        'convox_create_release_eu': 'build_convox_create_release-eu',
        'convox_create_release_staging': 'build_convox_create_release-staging',
        'convox_create_release_us': 'build_convox_create_release-us',
        'convox_pre_release_eu': 'deploy_eu_pre_release',
        'convox_pre_release_staging': 'deploy_staging_pre_release',
        'convox_pre_release_us': 'deploy_us_pre_release',
        'convox_promote_eu': 'deploy_eu_promote',
        'convox_promote_staging': 'deploy_staging_promote',
        'convox_promote_us': 'deploy_us_promote',
        'convox_request_approval_eu': 'deploy_eu_request_approval',
        'convox_request_approval_staging': 'deploy_staging_request_approval',
        'convox_request_approval_us': 'deploy_us_request_approval',
        'create_app_docker_manifest': 'build_create_app_docker_manifest',
        'create_enterprise_docker_manifest': 'build_create_enterprise_docker_manifest',
        'deploy_all': 'deploy_all_deploy_all',
        'deploy_docs': 'deploy_docs_deploy',
        'deploy_push_embedded_assets_to_s3': 'deploy_staging_push_assets',
        'developer_setup': 'ci_developer_setup',
        'docs_lint': 'ci_docs_lint',
        'ensure_branch_up_to_date': 'ci_ensure_branch_up_to_date',
        'install_app_npm_packages': 'ci_install_app_npm_packages',
        'install_docs_npm_packages': 'ci_install_docs_npm_packages',
        'install_gems_amd64': 'ci_install_gems',
        'js_lint': 'ci_js_lint',
        'merge_branch': 'post_deploy_merge',
        'patch_approval_jobs_status': 'post_deploy_patch_approval_jobs_status', # Verify exact name
        'patch_approve_deploy_all': 'post_deploy_patch_approve_deploy_all',
        'patch_approve_deploy_all': 'post_deploy_patch_approve_deploy_all',
        'postman_staging': 'deploy_staging_postman',
        'prettier': 'ci_prettier',
        'rspec_amd64': 'ci_rspec',
        'rspec_passed_amd64': 'ci_rspec_passed',
        'rswag_amd64': 'ci_rswag',
        'ruby_lint': 'ci_ruby_lint',
        'secrets': 'ci_secrets',
        'shellcheck': 'ci_shellcheck',
        'slack_ci_passed': 'ci_slack_ci_passed',
    }

    print(f"Total Old Jobs: {len(old_jobs)}")
    print(f"Total New Jobs: {len(new_jobs)}")

    mapped_old_jobs = set(job_mapping.keys())
    mapped_new_jobs = set(job_mapping.values())

    # Check for missing mappings (Old jobs not in mapping)
    unmapped_old = old_jobs - mapped_old_jobs
    if unmapped_old:
        print("\n--- Unmapped Old Jobs (Missing in Mapping) ---")
        for job in sorted(unmapped_old):
            print(job)

    # Check for missing new jobs (New jobs not in mapping)
    unmapped_new = new_jobs - mapped_new_jobs
    if unmapped_new:
        print("\n--- Unmapped New Jobs (Extras in New Config) ---")
        for job in sorted(unmapped_new):
            print(job)

    # Check if mapped jobs actually exist
    print("\n--- Verification ---")
    for old_j, new_j in job_mapping.items():
        if old_j not in old_jobs:
             print(f"WARNING: Mapped old job '{old_j}' does not exist in old config")
        if new_j not in new_jobs:
             print(f"WARNING: Mapped new job '{new_j}' does not exist in new config")



if __name__ == '__main__':
    main()