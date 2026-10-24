export type DesktopSettings = {
  onboardingCompleted: boolean;
  needsOnboarding: boolean;
  whisperModelPath: string | null;
  whisperModelReady: boolean;
  vadModelPath: string | null;
  vadModelReady: boolean;
  hyMt2ModelPath: string | null;
  hyMt2ModelReady: boolean;
  localTranslationRuntimeReady: boolean;
  translationProviderId: "none" | "deepl" | "deepseek" | "openai-compatible" | "hy-mt2-local";
  translationModel: string | null;
  translationBaseUrl: string | null;
  translationStyleInstruction: string;
  translationApiKeyConfigured: boolean;
  translationApiKeySource: "system" | "environment" | "saved" | "deferred" | "local" | null;
  credentialStore: string;
  credentialError: string | null;
  modelsDirectory: string;
  networkProxyMode: "environment" | "direct" | "custom";
  networkProxyUrl: string | null;
  modelMirrorUrl: string | null;
};

export type TranslationProfileSummary = { provider_id: string; model: string | null; base_url: string | null; style: string; readiness: string };
export type SaveDesktopSettingsRequest = {
  whisperModelPath: string | null; vadModelPath: string | null; hyMt2ModelPath: string | null;
  translationProviderId: string; translationModel: string | null; translationBaseUrl: string | null;
  translationStyleInstruction: string | null; apiKey: string | null; clearApiKey: boolean;
  networkProxyMode: string; networkProxyUrl: string | null; modelMirrorUrl: string | null;
  onboardingCompleted: boolean;
};
