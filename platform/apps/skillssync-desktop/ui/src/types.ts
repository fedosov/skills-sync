export type SkillLifecycleStatus = 'active' | 'archived'
export type SyncHealthStatus = 'ok' | 'failed' | 'syncing' | 'unknown'

export type SkillRecord = {
  id: string
  name: string
  scope: string
  workspace: string | null
  canonical_source_path: string
  target_paths: string[]
  status: SkillLifecycleStatus
  package_type: string
  skill_key: string
}

export type SyncSummary = {
  global_count: number
  project_count: number
  conflict_count: number
}

export type SyncMetadata = {
  status: SyncHealthStatus
  error: string | null
}

export type SyncState = {
  generated_at: string
  sync: SyncMetadata
  summary: SyncSummary
  skills: SkillRecord[]
}

export type SkillDetails = {
  skill: SkillRecord
  main_file_path: string
  main_file_exists: boolean
  main_file_body_preview: string | null
  main_file_body_preview_truncated: boolean
  last_modified_unix_seconds: number | null
}

export type MutationCommand =
  | 'archive_skill'
  | 'restore_skill'
  | 'delete_skill'
  | 'make_global'
