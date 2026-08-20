env = Rails.application.env_config.merge(env) if defined?(Rails.application) && Rails.application
ok = defined?(Warning) && true
name = defined?(config).nil?
