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

export type ModelCatalogItem = {
  id: string;
  kind: "whisper" | "vad" | "hy-mt2";
  name: string;
  fileName: string;
  sizeLabel: string;
  recommendedFor: string;
  sourceUrl: string;
};

export type ModelDownloadState = {
  modelId: string;
  status: "queued" | "downloading" | "cancelling" | "cancelled" | "done" | "failed";
  downloadedBytes: number;
  totalBytes: number | null;
  path: string | null;
  error: string | null;
  source: string | null;
  active: boolean;
};

export type DictionaryCatalogItem = {
  id: string;
  name: string;
  languagePair: string;
  versionLabel: string;
  sizeLabel: string;
  description: string;
  license: string;
  attribution: string;
  sourceUrl: string;
};

export type DictionaryDownloadState = {
  dictionaryId: string;
  status: "queued" | "resolving" | "downloading" | "done" | "failed";
  downloadedBytes: number;
  totalBytes: number | null;
  path: string | null;
  version: string | null;
  error: string | null;
  source: string | null;
};

export type DictionaryCredentialStatus = {
  providerId: "merriam-webster";
  providerName: string;
  configured: boolean;
  credentialStore: string;
};

export type DesktopSettingsSnapshot = {
  settings: DesktopSettings; catalog: ModelCatalogItem[]; downloads: ModelDownloadState[]; profiles: TranslationProfileSummary[];
};
export type DictionarySettingsSnapshot = {
  catalog: DictionaryCatalogItem[]; downloads: DictionaryDownloadState[]; credentials: DictionaryCredentialStatus[]; directory: string;
};
