export class AppError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly status: number,
  ) {
    super(message);
    this.name = "AppError";
  }
}

export class ConfigError extends AppError {
  constructor(
    message: string,
    readonly missing: string[] = [],
  ) {
    super("CONFIG_MISSING", message, 500);
    this.name = "ConfigError";
  }
}

export class LLMOutputInvalidError extends AppError {
  constructor(message: string) {
    super("LLM_OUTPUT_INVALID", message, 502);
    this.name = "LLMOutputInvalidError";
  }
}

export class ProviderError extends AppError {
  constructor(message: string) {
    super("PROVIDER_ERROR", message, 502);
    this.name = "ProviderError";
  }
}

export class JsonModeUnsupportedError extends Error {
  constructor(message = "JSON mode not supported by provider") {
    super(message);
    this.name = "JsonModeUnsupportedError";
  }
}
