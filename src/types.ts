export type AssistantState = 'idle' | 'listening' | 'thinking' | 'speaking' | 'confirm' | 'error';

export interface StatePayload {
  state: AssistantState;
  message: string;
  pending_command: string | null;
  minimized: boolean;
}

export interface AppConfig {
  llm_provider: string;
  gemini_api_key: string;
  groq_api_key: string;
  openai_api_key: string;
  openrouter_api_key: string;
  ollama_base_url: string;
  custom_api_key: string;
  custom_api_endpoint: string;
  model_name: string;
  tts_model_path: string;
  whisper_model_path: string;
  avatar_folder_path: string;
  assistant_name?: string;
  user_name?: string;
  personality_preset?: string;
  custom_personality?: string;
  avatar_size?: number;
  tts_engine?: string;
  edge_tts_voice?: string;
  edge_tts_rate?: string;
  elevenlabs_api_key?: string;
  elevenlabs_voice_id?: string;
  openai_speech_endpoint?: string;
  openai_speech_key?: string;
  openai_speech_voice?: string;
  active_avatar_preset?: string;
  active_cloned_voice_id?: string;
  fish_audio_api_key?: string;
  colab_clone_endpoint?: string;
}

export interface AvatarPresetMeta {
  id: string;
  name: string;
  thumbnail: string;
  frame_count: number;
  is_active: boolean;
}

export interface ClonedVoiceMeta {
  id: string;
  name: string;
  engine: string;
  remote_voice_id: string;
  sample_filename?: string;
  created_at: number;
  is_active: boolean;
}

export interface LipSyncPayload {
  is_mouth_open: boolean;
}
