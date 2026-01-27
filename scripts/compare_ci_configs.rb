#!/usr/bin/env ruby
# frozen_string_literal: true

require 'yaml'
require 'colorize'

class CIConfigComparator
  def initialize(old_dir = 'docspring/.circleci-orig', new_dir = 'docspring/.circleci')
    @old_dir = old_dir
    @new_dir = new_dir
    @old_configs = {}
    @new_configs = {}
    load_configs
  end

  def run
    puts '🔄 Comparing OLD vs NEW CircleCI configurations'.blue
    puts

    analyze_old_system
    analyze_new_system
    compare_systems
  end

  private

  def load_configs
    # Load OLD configs from .circleci-orig
    old_files = Dir.glob("#{@old_dir}/*.yml").reject do |f|
      basename = File.basename(f)
      basename == 'config.yml' || basename == 'test_config.yml' || basename == 'build_config.yml'
    end

    puts "📂 Loading OLD system configs from #{@old_dir}/...".yellow
    old_files.each do |file|
      filename = File.basename(file)
      puts "  - #{filename}"
      begin
        @old_configs[filename] = YAML.load_file(file, aliases: true)
      rescue StandardError => e
        puts "    ❌ Error loading #{filename}: #{e.message}".red
      end
    end

    # Load NEW configs (support split outputs: config.yml, main.yml, *_config.yml)
    puts '📂 Loading NEW cigen-generated configs...'.green
    Dir.glob(File.join(@new_dir, '*.yml')).each do |file|
      filename = File.basename(file)
      begin
        @new_configs[filename] = YAML.load_file(file, aliases: true)
        puts "  - #{filename}"
      rescue StandardError => e
        puts "❌ Error loading #{filename}: #{e.message}".red
      end
    end
    puts
  end

  def analyze_old_system
    puts '🏛️  OLD SYSTEM ANALYSIS'.yellow
    puts '=' * 50

    total_jobs = @old_configs.values.sum { |config| config['jobs']&.length || 0 }
    total_commands = @old_configs.values.sum { |config| config['commands']&.length || 0 }
    total_workflows = @old_configs.values.sum { |config| config['workflows']&.length || 0 }

    puts "📊 Totals across #{@old_configs.length} config files:"
    puts "   Jobs: #{total_jobs}"
    puts "   Commands: #{total_commands}"
    puts "   Workflows: #{total_workflows}"

    # Collect unique jobs, commands for deduplication analysis
    all_jobs = Set.new
    all_commands = Set.new

    @old_configs.each do |_filename, config|
      if config['jobs']
        all_jobs.merge(config['jobs'].keys)
      end
      if config['commands']
        all_commands.merge(config['commands'].keys)
      end
    end

    puts "   Unique Jobs: #{all_jobs.length}"
    puts "   Unique Commands: #{all_commands.length}"
    puts
  end

  def analyze_new_system
    puts '🆕 NEW SYSTEM ANALYSIS (cigen)'.green
    puts '=' * 50

    if @new_configs.empty?
      puts '❌ No new config found!'.red
      return
    end

    jobs = @new_configs.values.sum { |c| c['jobs']&.length || 0 }
    commands = @new_configs.values.sum { |c| c['commands']&.length || 0 }
    workflows = @new_configs.values.sum { |c| c['workflows']&.length || 0 }

    puts '📊 Single config.yml contains:'
    puts "   Jobs: #{jobs}"
    puts "   Commands: #{commands}"
    puts "   Workflows: #{workflows}"

    # Analyze structure
    # Parameters and orbs (aggregate unique keys)
    params = @new_configs.values.map { |c| c['parameters']&.keys || [] }.flatten.uniq
    puts "   Parameters: #{params.length}" unless params.empty?

    orbs = @new_configs.values.map { |c| c['orbs']&.keys || [] }.flatten.uniq
    puts "   Orbs: #{orbs.length}" unless orbs.empty?

    puts
  end

  def compare_systems
    puts '⚖️  COMPARISON'.blue
    puts '=' * 50

    if @new_configs.empty?
      puts '❌ Cannot compare - no new config loaded'.red
      return
    end

    # Calculate OLD totals
    old_jobs = @old_configs.values.sum { |config| config['jobs']&.length || 0 }
    old_commands = @old_configs.values.sum { |config| config['commands']&.length || 0 }
    old_workflows = @old_configs.values.sum { |config| config['workflows']&.length || 0 }

    # NEW totals
    new_jobs = @new_configs.values.sum { |c| c['jobs']&.length || 0 }
    new_commands = @new_configs.values.sum { |c| c['commands']&.length || 0 }
    new_workflows = @new_configs.values.sum { |c| c['workflows']&.length || 0 }

    puts '📊 COMPARISON:'
    puts "   Jobs:      #{old_jobs.to_s.rjust(3)} (old) vs #{new_jobs.to_s.ljust(3)} (new) | #{format_diff(old_jobs, new_jobs)}"
    puts "   Commands:  #{old_commands.to_s.rjust(3)} (old) vs #{new_commands.to_s.ljust(3)} (new) | #{format_diff(old_commands, new_commands)}"
    puts "   Workflows: #{old_workflows.to_s.rjust(3)} (old) vs #{new_workflows.to_s.ljust(3)} (new) | #{format_diff(old_workflows, new_workflows)}"

    # Check for missing critical job types
    puts "\n🔍 CRITICAL CHECKS:"
    check_critical_patterns

    # Cache operations check
    puts "\n💾 CACHE ANALYSIS:"
    compare_cache_usage

    puts
    if new_jobs == old_jobs && new_commands >= old_commands && new_workflows == old_workflows
      puts '✅ Structure looks good! Job counts match.'.green
    elsif new_jobs < old_jobs * 0.8 # More than 20% fewer jobs might indicate missing workflows
      puts '⚠️  Significant difference in job count - check if workflows are missing'.yellow
    else
      puts 'ℹ️  Differences detected - manual review recommended'.blue
    end

    puts
    puts '📋 Detailed Job Comparison:'.blue
    old_job_names = @old_configs.values.flat_map { |config| config['jobs']&.keys || [] }.sort
    new_job_names = @new_configs.values.flat_map { |config| config['jobs']&.keys || [] }.sort

    missing_jobs = old_job_names - new_job_names
    added_jobs = new_job_names - old_job_names

    if missing_jobs.any?
      puts "❌ Missing Jobs (#{missing_jobs.length}):".red
      missing_jobs.each { |j| puts "   - #{j}" }
    else
      puts "✅ No missing jobs (exact name match)".green
    end

    puts

    if added_jobs.any?
      puts "mw Added Jobs (#{added_jobs.length}):".green
      added_jobs.each { |j| puts "   - #{j}" }
    else
      puts "No new jobs".yellow
    end
  end

  def format_diff(old_val, new_val)
    diff = new_val - old_val
    if diff == 0
      '✅ same'.green
    elsif diff > 0
      "+#{diff}".green
    else
      diff.to_s.red
    end
  end

  def check_critical_patterns
    old_job_names = @old_configs.values.flat_map { |config| config['jobs']&.keys || [] }
    new_job_names = @new_configs.values.flat_map { |config| config['jobs']&.keys || [] }

    critical_patterns = [
      { name: 'Deploy jobs', pattern: /deploy/ },
      { name: 'Build jobs', pattern: /build/ },
      { name: 'Test jobs', pattern: /test|spec/ },
      { name: 'Approval jobs', pattern: /approv/ },
    ]

    critical_patterns.each do |check|
      old_matches = old_job_names.grep(check[:pattern])
      new_matches = new_job_names.grep(check[:pattern])

      status = if new_matches.length == old_matches.length
        '✅'
      elsif new_matches.empty? && !old_matches.empty?
        '❌'
      else
        '⚠️'
      end

      puts "   #{status} #{check[:name]}: #{old_matches.length} → #{new_matches.length}"

      if new_matches.empty? && !old_matches.empty?
        puts "     Missing: #{old_matches.join(', ')}"
      end
    end
  end

  def compare_cache_usage
    # Count cache operations in old system
    old_cache_ops = { restore: 0, save: 0 }
    @old_configs.values.each do |config|
      next unless config['jobs']

      config['jobs'].values.each do |job|
        cache_ops = count_cache_operations(job['steps'] || [])
        old_cache_ops[:restore] += cache_ops[:restore]
        old_cache_ops[:save] += cache_ops[:save]
      end
    end

    # Count cache operations in new system
    new_cache_ops = { restore: 0, save: 0 }
    @new_configs.values.each do |cfg|
      cfg['jobs']&.values&.each do |job|
        cache_ops = count_cache_operations(job['steps'] || [])
        new_cache_ops[:restore] += cache_ops[:restore]
        new_cache_ops[:save] += cache_ops[:save]
      end
    end

    puts "   Restore operations: #{old_cache_ops[:restore]} → #{new_cache_ops[:restore]} | #{format_diff(
      old_cache_ops[:restore],
      new_cache_ops[:restore]
    )}"
    puts "   Save operations:    #{old_cache_ops[:save]} → #{new_cache_ops[:save]} | #{format_diff(old_cache_ops[:save], new_cache_ops[:save])}"
  end

  def count_cache_operations(steps)
    restore_count = 0
    save_count = 0

    steps.each do |step|
      next unless step.is_a?(Hash)

      if step['restore_cache'] || step.keys.any? { |k| k.to_s.include?('restore') && k.to_s.include?('cache') }
        restore_count += 1
      end
      if step['save_cache'] || step.keys.any? { |k| k.to_s.include?('save') && k.to_s.include?('cache') }
        save_count += 1
      end
    end

    { restore: restore_count, save: save_count }
  end
end

if __FILE__ == $0
  CIConfigComparator.new.run
end
