#!/usr/bin/env python3
"""
CI Config Comparison Framework

Bidirectional, hierarchical comparison of CircleCI configs.
Compares OLD (Ruby-generated) ↔ NEW (cigen-generated) configs.

Structure:
  - Workflows → Jobs → Steps → ...
  - Stop at unmapped items (don't go deeper until current level is mapped)
  - Mapping types: one-to-one, addition, removal

Usage:
    python scripts/ci_config_comparison/compare.py [--old PATH] [--new PATH]
"""

import argparse
import yaml
from pathlib import Path
from dataclasses import dataclass, field
from enum import Enum, auto
from typing import Any, Optional


# =============================================================================
# Core Types
# =============================================================================


class MappingType(Enum):
    """Type of mapping between old and new items."""
    ONE_TO_ONE = auto()   # OLD X ↔ NEW Y
    ADDITION = auto()     # NEW Y is intentionally new (no OLD equivalent)
    REMOVAL = auto()      # OLD X is intentionally removed (no NEW equivalent)


@dataclass
class Mapping:
    """A mapping between an old item and a new item."""
    old_name: Optional[str]  # None for additions
    new_name: Optional[str]  # None for removals
    mapping_type: MappingType
    comment: str = ""  # User-provided explanation

    def __post_init__(self):
        if self.mapping_type == MappingType.ONE_TO_ONE:
            assert self.old_name and self.new_name
        elif self.mapping_type == MappingType.ADDITION:
            assert self.new_name and not self.old_name
        elif self.mapping_type == MappingType.REMOVAL:
            assert self.old_name and not self.new_name


@dataclass
class ComparisonResult:
    """Result of comparing items at one level."""
    level: str  # e.g., "workflows", "jobs", "steps"
    mapped: list[Mapping] = field(default_factory=list)
    unmapped_old: list[str] = field(default_factory=list)  # In OLD but not mapped
    unmapped_new: list[str] = field(default_factory=list)  # In NEW but not mapped

    @property
    def is_fully_mapped(self) -> bool:
        return len(self.unmapped_old) == 0 and len(self.unmapped_new) == 0

    @property
    def has_unmapped(self) -> bool:
        return len(self.unmapped_old) > 0 or len(self.unmapped_new) > 0


# =============================================================================
# Mappings Registry
# =============================================================================


class MappingsRegistry:
    """
    Registry of all approved mappings.

    This is where user-approved mappings are stored.
    The registry is hierarchical: workflows → jobs → steps → etc.
    """

    def __init__(self):
        self.workflows: list[Mapping] = []
        self.jobs: dict[str, list[Mapping]] = {}  # workflow_key -> job mappings
        self.steps: dict[str, list[Mapping]] = {}  # job_key -> step mappings
        self.commands: list[Mapping] = []
        self.parameters: list[Mapping] = []
        self.orbs: list[Mapping] = []

    def add_workflow_mapping(self, old: Optional[str], new: Optional[str],
                            mapping_type: MappingType, comment: str = ""):
        self.workflows.append(Mapping(old, new, mapping_type, comment))

    def add_job_mapping(self, workflow_key: str, old: Optional[str], new: Optional[str],
                       mapping_type: MappingType, comment: str = ""):
        if workflow_key not in self.jobs:
            self.jobs[workflow_key] = []
        self.jobs[workflow_key].append(Mapping(old, new, mapping_type, comment))

    def add_step_mapping(self, job_key: str, old: Optional[str], new: Optional[str],
                        mapping_type: MappingType, comment: str = ""):
        if job_key not in self.steps:
            self.steps[job_key] = []
        self.steps[job_key].append(Mapping(old, new, mapping_type, comment))

    def add_command_mapping(self, old: Optional[str], new: Optional[str],
                           mapping_type: MappingType, comment: str = ""):
        self.commands.append(Mapping(old, new, mapping_type, comment))

    def add_parameter_mapping(self, old: Optional[str], new: Optional[str],
                             mapping_type: MappingType, comment: str = ""):
        self.parameters.append(Mapping(old, new, mapping_type, comment))

    def add_orb_mapping(self, old: Optional[str], new: Optional[str],
                       mapping_type: MappingType, comment: str = ""):
        self.orbs.append(Mapping(old, new, mapping_type, comment))

    def get_workflow_mappings(self) -> list[Mapping]:
        return self.workflows

    def get_job_mappings(self, workflow_key: str) -> list[Mapping]:
        return self.jobs.get(workflow_key, [])

    def get_step_mappings(self, job_key: str) -> list[Mapping]:
        return self.steps.get(job_key, [])


# =============================================================================
# Config Loader
# =============================================================================


def load_yaml(path: Path) -> dict:
    """Load a YAML file."""
    with open(path) as f:
        return yaml.safe_load(f)


@dataclass
class ConfigPair:
    """A pair of configs to compare."""
    old_path: Path
    new_path: Path
    old_config: dict = field(default_factory=dict)
    new_config: dict = field(default_factory=dict)

    def load(self):
        self.old_config = load_yaml(self.old_path)
        self.new_config = load_yaml(self.new_path)


# =============================================================================
# Extractors - Get items from configs
# =============================================================================


def get_workflow_names(config: dict) -> set[str]:
    """Get workflow names from a config."""
    return set(config.get("workflows", {}).keys())


def get_workflow_jobs(config: dict, workflow_name: str) -> list[str]:
    """Get job names from a workflow."""
    workflow = config.get("workflows", {}).get(workflow_name, {})
    jobs = workflow.get("jobs", [])
    result = []
    for job in jobs:
        if isinstance(job, str):
            result.append(job)
        elif isinstance(job, dict):
            result.append(list(job.keys())[0])
    return result


def get_job_names(config: dict) -> set[str]:
    """Get job definition names from a config."""
    return set(config.get("jobs", {}).keys())


def get_job_steps(config: dict, job_name: str) -> list[dict]:
    """Get steps from a job definition."""
    job = config.get("jobs", {}).get(job_name, {})
    return job.get("steps", [])


def get_step_identifier(step: Any) -> str:
    """Get a unique identifier for a step."""
    if isinstance(step, str):
        return step
    if isinstance(step, dict):
        keys = list(step.keys())
        if keys:
            key = keys[0]
            if key == "run" and isinstance(step[key], dict):
                name = step[key].get("name", "")
                if name:
                    return f"run:{name}"
            return key
    return str(step)


def get_command_names(config: dict) -> set[str]:
    """Get command names from a config."""
    return set(config.get("commands", {}).keys())


def get_parameter_names(config: dict) -> set[str]:
    """Get parameter names from a config."""
    return set(config.get("parameters", {}).keys())


def get_orb_names(config: dict) -> set[str]:
    """Get orb names from a config."""
    return set(config.get("orbs", {}).keys())


# =============================================================================
# Comparator Engine
# =============================================================================


class Comparator:
    """
    Hierarchical bidirectional comparator.

    Compares OLD ↔ NEW at each level, stopping at unmapped items.
    """

    def __init__(self, config_pair: ConfigPair, registry: MappingsRegistry):
        self.old = config_pair.old_config
        self.new = config_pair.new_config
        self.registry = registry
        self.results: dict[str, ComparisonResult] = {}

    def compare_level(self, level: str, old_items: set[str], new_items: set[str],
                     mappings: list[Mapping]) -> ComparisonResult:
        """Compare items at a single level using the provided mappings."""
        result = ComparisonResult(level=level)

        # Track which items are covered by mappings
        mapped_old = set()
        mapped_new = set()

        for mapping in mappings:
            result.mapped.append(mapping)
            if mapping.old_name:
                mapped_old.add(mapping.old_name)
            if mapping.new_name:
                mapped_new.add(mapping.new_name)

        # Find unmapped items (only those that actually exist in the configs)
        result.unmapped_old = sorted(old_items - mapped_old)
        result.unmapped_new = sorted(new_items - mapped_new)

        return result

    def compare_workflows(self) -> ComparisonResult:
        """Compare workflows."""
        old_workflows = get_workflow_names(self.old)
        new_workflows = get_workflow_names(self.new)
        mappings = self.registry.get_workflow_mappings()

        result = self.compare_level("workflows", old_workflows, new_workflows, mappings)
        self.results["workflows"] = result
        return result

    def compare_jobs_in_workflow(self, old_workflow: Optional[str], new_workflow: Optional[str],
                                 workflow_key: str) -> ComparisonResult:
        """Compare jobs within a mapped workflow pair."""
        old_jobs = set(get_workflow_jobs(self.old, old_workflow)) if old_workflow else set()
        new_jobs = set(get_workflow_jobs(self.new, new_workflow)) if new_workflow else set()

        mappings = self.registry.get_job_mappings(workflow_key)

        result = self.compare_level(f"jobs:{workflow_key}", old_jobs, new_jobs, mappings)
        self.results[f"jobs:{workflow_key}"] = result
        return result

    def compare_steps_in_job(self, old_job: Optional[str], new_job: Optional[str],
                            job_key: str) -> ComparisonResult:
        """Compare steps within a mapped job pair."""
        old_steps = get_job_steps(self.old, old_job) if old_job else []
        new_steps = get_job_steps(self.new, new_job) if new_job else []

        old_step_ids = set(get_step_identifier(s) for s in old_steps)
        new_step_ids = set(get_step_identifier(s) for s in new_steps)

        mappings = self.registry.get_step_mappings(job_key)

        result = self.compare_level(f"steps:{job_key}", old_step_ids, new_step_ids, mappings)
        self.results[f"steps:{job_key}"] = result
        return result

    def compare_commands(self) -> ComparisonResult:
        """Compare command definitions."""
        old_cmds = get_command_names(self.old)
        new_cmds = get_command_names(self.new)
        mappings = self.registry.commands

        result = self.compare_level("commands", old_cmds, new_cmds, mappings)
        self.results["commands"] = result
        return result

    def compare_parameters(self) -> ComparisonResult:
        """Compare parameters."""
        old_params = get_parameter_names(self.old)
        new_params = get_parameter_names(self.new)
        mappings = self.registry.parameters

        result = self.compare_level("parameters", old_params, new_params, mappings)
        self.results["parameters"] = result
        return result

    def compare_orbs(self) -> ComparisonResult:
        """Compare orbs."""
        old_orbs = get_orb_names(self.old)
        new_orbs = get_orb_names(self.new)
        mappings = self.registry.orbs

        result = self.compare_level("orbs", old_orbs, new_orbs, mappings)
        self.results["orbs"] = result
        return result

    def run_hierarchical(self) -> dict[str, ComparisonResult]:
        """
        Run hierarchical comparison.

        Stops at each level if there are unmapped items.
        """
        # Level 1: Workflows
        wf_result = self.compare_workflows()
        if wf_result.has_unmapped:
            return self.results

        # Level 2: Jobs (for each mapped workflow)
        all_jobs_mapped = True
        for mapping in wf_result.mapped:
            if mapping.mapping_type == MappingType.ONE_TO_ONE:
                # Use a key that works for lookups
                workflow_key = f"{mapping.old_name}:{mapping.new_name}"
                job_result = self.compare_jobs_in_workflow(
                    mapping.old_name, mapping.new_name, workflow_key
                )
                if job_result.has_unmapped:
                    all_jobs_mapped = False
            elif mapping.mapping_type == MappingType.ADDITION:
                # New workflow with no old equivalent - compare against empty
                workflow_key = f"_:{mapping.new_name}"
                job_result = self.compare_jobs_in_workflow(
                    None, mapping.new_name, workflow_key
                )
                if job_result.has_unmapped:
                    all_jobs_mapped = False
            elif mapping.mapping_type == MappingType.REMOVAL:
                # Old workflow with no new equivalent - compare against empty
                workflow_key = f"{mapping.old_name}:_"
                job_result = self.compare_jobs_in_workflow(
                    mapping.old_name, None, workflow_key
                )
                if job_result.has_unmapped:
                    all_jobs_mapped = False

        if not all_jobs_mapped:
            return self.results

        # Level 3: Steps (for each mapped job)
        for wf_mapping in wf_result.mapped:
            if wf_mapping.mapping_type == MappingType.ONE_TO_ONE:
                workflow_key = f"{wf_mapping.old_name}:{wf_mapping.new_name}"
            elif wf_mapping.mapping_type == MappingType.ADDITION:
                workflow_key = f"_:{wf_mapping.new_name}"
            elif wf_mapping.mapping_type == MappingType.REMOVAL:
                workflow_key = f"{wf_mapping.old_name}:_"
            else:
                continue

            job_mappings = self.registry.get_job_mappings(workflow_key)
            for job_mapping in job_mappings:
                if job_mapping.mapping_type == MappingType.ONE_TO_ONE:
                    job_key = f"{job_mapping.old_name}:{job_mapping.new_name}"
                    self.compare_steps_in_job(
                        job_mapping.old_name, job_mapping.new_name, job_key
                    )

        # Also compare top-level items
        self.compare_commands()
        self.compare_parameters()
        self.compare_orbs()

        return self.results


# =============================================================================
# Reporter
# =============================================================================


class Reporter:
    """Reports comparison results."""

    def __init__(self, results: dict[str, ComparisonResult]):
        self.results = results

    def print_result(self, result: ComparisonResult):
        """Print a single comparison result."""
        print(f"\n{'=' * 80}")
        print(f"LEVEL: {result.level}")
        print('=' * 80)

        if result.mapped:
            print(f"\nMAPPED ({len(result.mapped)}):")
            for m in result.mapped:
                if m.mapping_type == MappingType.ONE_TO_ONE:
                    print(f"  ↔ {m.old_name} ↔ {m.new_name}")
                elif m.mapping_type == MappingType.ADDITION:
                    print(f"  + {m.new_name} (addition)")
                elif m.mapping_type == MappingType.REMOVAL:
                    print(f"  - {m.old_name} (removal)")
                if m.comment:
                    print(f"      Comment: {m.comment}")

        if result.unmapped_old:
            print(f"\nUNMAPPED IN OLD ({len(result.unmapped_old)}):")
            for name in result.unmapped_old:
                print(f"  ? {name}")

        if result.unmapped_new:
            print(f"\nUNMAPPED IN NEW ({len(result.unmapped_new)}):")
            for name in result.unmapped_new:
                print(f"  ? {name}")

        if result.is_fully_mapped:
            print(f"\n✓ Level '{result.level}' is fully mapped")
        else:
            print(f"\n✗ Level '{result.level}' has unmapped items - STOPPING HERE")

    def print_all(self):
        """Print all results."""
        for level, result in self.results.items():
            self.print_result(result)

    def print_summary(self):
        """Print a summary."""
        print(f"\n{'=' * 80}")
        print("SUMMARY")
        print('=' * 80)

        total_unmapped = 0
        for level, result in self.results.items():
            status = "✓" if result.is_fully_mapped else "✗"
            unmapped = len(result.unmapped_old) + len(result.unmapped_new)
            total_unmapped += unmapped
            print(f"  {status} {level}: {len(result.mapped)} mapped, {unmapped} unmapped")

        print()
        if total_unmapped == 0:
            print("All items are mapped!")
        else:
            print(f"Total unmapped items: {total_unmapped}")


# =============================================================================
# Main
# =============================================================================


def create_default_registry() -> MappingsRegistry:
    """
    Create registry with approved mappings.

    THIS IS WHERE USER-APPROVED MAPPINGS GO.
    Each mapping should be added here after user approval.
    """
    registry = MappingsRegistry()

    # =========================================================================
    # WORKFLOW MAPPINGS (user-approved)
    # =========================================================================


    # =========================================================================
    # JOB MAPPINGS (user-approved, grouped by workflow key)
    # =========================================================================


    # =========================================================================
    # STEP MAPPINGS (user-approved, grouped by job key)
    # =========================================================================


    # =========================================================================
    # COMMAND MAPPINGS (user-approved)
    # =========================================================================


    # =========================================================================
    # PARAMETER MAPPINGS (user-approved)
    # =========================================================================


    # =========================================================================
    # ORB MAPPINGS (user-approved)
    # =========================================================================


    return registry


def main():
    parser = argparse.ArgumentParser(description="Compare CircleCI configs")
    parser.add_argument("--old", type=Path,
                       default=Path("/Users/ndbroadbent/code/docspring/.circleci/test_and_deploy_config.yml"),
                       help="Path to old (Ruby-generated) config")
    parser.add_argument("--new", type=Path,
                       default=Path("docspring/.circleci/main.yml"),
                       help="Path to new (cigen-generated) config")
    args = parser.parse_args()

    print(f"OLD: {args.old}")
    print(f"NEW: {args.new}")

    # Load configs
    config_pair = ConfigPair(args.old, args.new)
    config_pair.load()

    # Create registry with approved mappings
    registry = create_default_registry()

    # Run comparison
    comparator = Comparator(config_pair, registry)
    results = comparator.run_hierarchical()

    # Report
    reporter = Reporter(results)
    reporter.print_all()
    reporter.print_summary()


if __name__ == "__main__":
    main()
