export type TranslationCredentialCheck = {
  providerId: string;
  providerName: string;
  storedInSystem: boolean;
  availableFromEnvironment: boolean;
  credentialStore: string;
};

export type AsrCredentialStatus = {
  providerId: "gemini-transcribe";
  providerName: string;
  configured: boolean;
  credentialStore: string;
};

export type ProviderAction = "test" | "check" | "save" | "delete";
