#!/usr/bin/env ruby
# frozen_string_literal: true

require 'yaml'
require 'colorize'

class CIConfigValidator
  def initialize(config_dir = 'docspring/.circleci')
    @config_dir = config_dir
    @configs = {}
    load_configs
  end

  def run
    puts "🔍 Validating CircleCI configurations in #{@config_dir}".blue
    puts

    @configs.each do |file, data|
      analyze_config(file, data)
    end

    summary
  end

  private

  def load_configs
    config_files = Dir.glob("#{@config_dir}/*.yml").reject { |f| File.basename(f) == 'config.yml' }

    config_files.each do |file|
      filename = File.basename(file)
      puts "📂 Loading #{filename}..."
      begin
        @configs[filename] = YAML.load_file(file, aliases: true)
      rescue StandardError => e
        puts "❌ Error loading #{filename}: #{e.message}".red
      end
    end
    puts
  end

  def analyze_config(filename, config)
    puts "📊 Analyzing #{filename}".yellow
    puts '=' * 50

    # Basic structure
    puts "Version: #{config['version'] || 'NOT SET'}"

    # Parameters
    if config['parameters']
      puts "Parameters: #{config['parameters'].keys.length}"
      config['parameters'].each do |name, param|
        puts "  - #{name}: #{param['type']} (default: #{param['default'] || 'none'})"
      end
    else
      puts 'Parameters: 0'
    end

    # Orbs
    if config['orbs']
      puts "Orbs: #{config['orbs'].length}"
      config['orbs'].each { |name, version| puts "  - #{name}: #{version}" }
    else
      puts 'Orbs: 0'
    end

    # Commands
    if config['commands']
      puts "Commands: #{config['commands'].length}"
      config['commands'].each do |name, cmd|
        step_count = cmd['steps']&.length || 0
        param_count = cmd['parameters']&.length || 0
        puts "  - #{name}: #{step_count} steps, #{param_count} parameters"
      end
    else
      puts 'Commands: 0'
    end

    # Jobs
    if config['jobs']
      puts "Jobs: #{config['jobs'].length}"
      config['jobs'].each do |name, job|
        step_count = job['steps']&.length || 0
        puts "  - #{name}: #{step_count} steps"

        # Check for cache operations
        cache_ops = count_cache_operations(job['steps'] || [])
        if cache_ops[:restore] > 0 || cache_ops[:save] > 0
          puts "    Cache: #{cache_ops[:restore]} restore, #{cache_ops[:save]} save"
        end
      end
    else
      puts 'Jobs: 0'
    end

    # Workflows
    if config['workflows']
      puts "Workflows: #{config['workflows'].length}"
      config['workflows'].each do |name, workflow|
        job_count = workflow['jobs']&.length || 0
        puts "  - #{name}: #{job_count} jobs"

        # Check for conditional logic
        if workflow['when'] || workflow['unless']
          puts "    Has conditions: #{workflow['when'] ? 'when' : ''}#{workflow['unless'] ? 'unless' : ''}"
        end
      end
    else
      puts 'Workflows: 0'
    end

    puts
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

  def summary
    puts '📈 SUMMARY'.green
    puts '=' * 50

    total_jobs = @configs.values.sum { |config| config['jobs']&.length || 0 }
    total_commands = @configs.values.sum { |config| config['commands']&.length || 0 }
    total_workflows = @configs.values.sum { |config| config['workflows']&.length || 0 }

    puts 'Total across all configs:'
    puts "  Jobs: #{total_jobs}"
    puts "  Commands: #{total_commands}"
    puts "  Workflows: #{total_workflows}"

    # Look for key patterns
    puts "\nKey patterns found:"

    # Check for deployment jobs
    deploy_jobs = @configs.values.flat_map do |config|
      (config['jobs'] || {}).keys.select { |name| name.to_s.downcase.include?('deploy') }
    end
    puts "  Deployment jobs: #{deploy_jobs.length} (#{deploy_jobs.join(', ')})"

    # Check for approval steps
    approval_jobs = @configs.values.flat_map do |config|
      (config['jobs'] || {}).select do |name, job|
        job['type'] == 'approval' || name.to_s.include?('approval')
      end.keys
    end
    puts "  Approval jobs: #{approval_jobs.length} (#{approval_jobs.join(', ')})"

    # Check for build jobs
    build_jobs = @configs.values.flat_map do |config|
      (config['jobs'] || {}).keys.select { |name| name.to_s.downcase.include?('build') }
    end
    puts "  Build jobs: #{build_jobs.length} (#{build_jobs.join(', ')})"

    # Check for test jobs
    test_jobs = @configs.values.flat_map do |config|
      (config['jobs'] || {}).keys.select do |name|
        name.to_s.downcase.include?('test') || name.to_s.downcase.include?('spec')
      end
    end
    puts "  Test jobs: #{test_jobs.length} (#{test_jobs.join(', ')})"

    puts
    puts '✅ Analysis complete!'.green
  end
end

if __FILE__ == $0
  CIConfigValidator.new.run
end
