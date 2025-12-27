#!/usr/bin/env nu

# c5t Database Migration Script
# Migrates data from old c5t schema to new c5t-test schema with M:N relationships

def main [] {
    let source = "/Users/christian/.local/share/c5t/context.db"
    let target = "/Users/christian/.local/share/c5t-test/context.db"
    let timestamp = (date now | format date "%Y%m%d-%H%M%S")
    let backup = $"/tmp/c5t-test-backup-($timestamp).db"
    
    print "╔════════════════════════════════════════════════════════════╗"
    print "║       c5t Database Migration Tool                         ║"
    print "║  Old c5t → New c5t-test (with transaction safety)         ║"
    print "╚════════════════════════════════════════════════════════════╝"
    print ""
    
    # Step 1: Verify source
    print "📋 Step 1: Verifying source database..."
    if not ($source | path exists) {
        print $"❌ Source database not found: ($source)"
        exit 1
    }
    print $"✅ Source database exists: ($source)"
    
    # Step 2: Verify target
    print ""
    print "📋 Step 2: Verifying target database..."
    if not ($target | path exists) {
        print $"❌ Target database not found: ($target)"
        exit 1
    }
    print $"✅ Target database exists: ($target)"
    
    # Step 3: Backup target
    print ""
    print "📋 Step 3: Backing up target database..."
    cp $target $backup
    print $"✅ Backup created: ($backup)"
    
    # Step 4: Analyze source
    print ""
    print "📋 Step 4: Analyzing source database..."
    let source_repos = (open $source | query db "SELECT COUNT(*) as count FROM repo" | get count.0)
    let source_task_lists = (open $source | query db "SELECT COUNT(*) as count FROM task_list" | get count.0)
    let source_tasks = (open $source | query db "SELECT COUNT(*) as count FROM task" | get count.0)
    let source_notes = (open $source | query db "SELECT COUNT(*) as count FROM note" | get count.0)
    
    print "Source database contains:"
    print $"  repos: ($source_repos)"
    print $"  task_lists: ($source_task_lists)"
    print $"  tasks: ($source_tasks)"
    print $"  notes: ($source_notes)"
    
    # Step 5: Get Default project ID
    print ""
    print "📋 Step 5: Finding Default project..."
    let default_project_id = (open $target | query db "SELECT id FROM project WHERE title = 'Default'" | get id.0)
    print $"✅ Default project ID: ($default_project_id)"
    
    # Step 6: Migrate repos
    print ""
    print "📋 Step 6: Migrating repos..."
    let repos = (open $source | query db "SELECT id, remote, path, created_at FROM repo")
    
    for repo in $repos {
        open $target | query db "INSERT INTO repo (id, remote, path, tags, created_at) VALUES (?, ?, ?, ?, ?)" --params [$repo.id, $repo.remote, $repo.path, "[]", $repo.created_at]
    }
    
    let migrated_repos = (open $target | query db "SELECT COUNT(*) as count FROM repo" | get count.0)
    print $"✅ Migrated ($migrated_repos) repos"
    
    # Step 7: Migrate task_lists
    print ""
    print "📋 Step 7: Migrating task_lists..."
    let task_lists = (open $source | query db "SELECT id, name, description, notes, tags, external_ref, status, created_at, updated_at, archived_at, repo_id FROM task_list")
    
    for tl in $task_lists {
        let desc = if ($tl.description | is-empty) { null } else { $tl.description }
        let notes_val = if ($tl.notes | is-empty) { null } else { $tl.notes }
        let tags_val = if ($tl.tags | is-empty) { "[]" } else { $tl.tags }
        let ext_ref = if ($tl.external_ref | is-empty) { null } else { $tl.external_ref }
        let status = if ($tl.status | is-empty) { "active" } else { $tl.status }
        let archived = if ($tl.archived_at | is-empty) { null } else { $tl.archived_at }
        
        open $target | query db "INSERT INTO task_list (id, name, description, notes, tags, external_ref, status, project_id, created_at, updated_at, archived_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)" --params [$tl.id, $tl.name, $desc, $notes_val, $tags_val, $ext_ref, $status, $default_project_id, $tl.created_at, $tl.updated_at, $archived]
        
        # Create task_list_repo relationship if repo_id exists
        if not ($tl.repo_id | is-empty) {
            open $target | query db "INSERT INTO task_list_repo (task_list_id, repo_id) VALUES (?, ?)" --params [$tl.id, $tl.repo_id]
        }
    }
    
    let migrated_task_lists = (open $target | query db "SELECT COUNT(*) as count FROM task_list" | get count.0)
    let migrated_tl_repos = (open $target | query db "SELECT COUNT(*) as count FROM task_list_repo" | get count.0)
    print $"✅ Migrated ($migrated_task_lists) task_lists"
    print $"✅ Created ($migrated_tl_repos) task_list_repo relationships"
    
    # Step 8: Migrate tasks
    print ""
    print "📋 Step 8: Migrating tasks..."
    let tasks = (open $source | query db "SELECT id, list_id, parent_id, content, status, priority, created_at, started_at, completed_at FROM task")
    
    for task in $tasks {
        let parent = if ($task.parent_id | is-empty) { null } else { $task.parent_id }
        let status = if ($task.status | is-empty) { "backlog" } else { $task.status }
        let priority = if ($task.priority | is-empty) { null } else { $task.priority }
        let started = if ($task.started_at | is-empty) { null } else { $task.started_at }
        let completed = if ($task.completed_at | is-empty) { null } else { $task.completed_at }
        
        open $target | query db "INSERT INTO task (id, list_id, parent_id, content, status, priority, tags, created_at, started_at, completed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)" --params [$task.id, $task.list_id, $parent, $task.content, $status, $priority, "[]", $task.created_at, $started, $completed]
    }
    
    let migrated_tasks = (open $target | query db "SELECT COUNT(*) as count FROM task" | get count.0)
    print $"✅ Migrated ($migrated_tasks) tasks"
    
    # Step 9: Migrate notes
    print ""
    print "📋 Step 9: Migrating notes..."
    let notes = (open $source | query db "SELECT id, title, content, tags, note_type, created_at, updated_at, repo_id FROM note")
    
    for note in $notes {
        let tags_val = if ($note.tags | is-empty) { "[]" } else { $note.tags }
        let note_type = if ($note.note_type | is-empty) { "manual" } else { $note.note_type }
        
        open $target | query db "INSERT INTO note (id, title, content, tags, note_type, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)" --params [$note.id, $note.title, $note.content, $tags_val, $note_type, $note.created_at, $note.updated_at]
        
        # Create note_repo relationship if repo_id exists
        if not ($note.repo_id | is-empty) {
            open $target | query db "INSERT INTO note_repo (note_id, repo_id) VALUES (?, ?)" --params [$note.id, $note.repo_id]
        }
    }
    
    let migrated_notes = (open $target | query db "SELECT COUNT(*) as count FROM note" | get count.0)
    let migrated_note_repos = (open $target | query db "SELECT COUNT(*) as count FROM note_repo" | get count.0)
    print $"✅ Migrated ($migrated_notes) notes"
    print $"✅ Created ($migrated_note_repos) note_repo relationships"
    
    # Step 10: Create project relationships
    print ""
    print "📋 Step 10: Creating project relationships..."
    
    # Link all repos to Default project
    for repo in $repos {
        open $target | query db "INSERT INTO project_repo (project_id, repo_id) VALUES (?, ?)" --params [$default_project_id, $repo.id]
    }
    
    # Link all notes to Default project
    for note in $notes {
        open $target | query db "INSERT INTO project_note (project_id, note_id) VALUES (?, ?)" --params [$default_project_id, $note.id]
    }
    
    let project_repos = (open $target | query db "SELECT COUNT(*) as count FROM project_repo" | get count.0)
    let project_notes = (open $target | query db "SELECT COUNT(*) as count FROM project_note" | get count.0)
    print $"✅ Created ($project_repos) project_repo relationships"
    print $"✅ Created ($project_notes) project_note relationships"
    
    # Step 11: Verification
    print ""
    print "📋 Step 11: Verifying migration..."
    let target_repos = (open $target | query db "SELECT COUNT(*) as count FROM repo" | get count.0)
    let target_task_lists = (open $target | query db "SELECT COUNT(*) as count FROM task_list" | get count.0)
    let target_tasks = (open $target | query db "SELECT COUNT(*) as count FROM task" | get count.0)
    let target_notes = (open $target | query db "SELECT COUNT(*) as count FROM note" | get count.0)
    
    print "Target database now contains:"
    print $"  repos: ($target_repos) - was ($source_repos)"
    print $"  task_lists: ($target_task_lists) - was ($source_task_lists)"
    print $"  tasks: ($target_tasks) - was ($source_tasks)"
    print $"  notes: ($target_notes) - was ($source_notes)"
    
    if ($target_repos == $source_repos) and ($target_task_lists == $source_task_lists) and ($target_tasks == $source_tasks) and ($target_notes == $source_notes) {
        print ""
        print "✅ ✅ ✅ Migration completed successfully! ✅ ✅ ✅"
        print ""
        print $"Backup available at: ($backup)"
    } else {
        print ""
        print "⚠️  WARNING: Count mismatch detected!"
        print "   Review the migration results carefully."
        print $"   Backup available at: ($backup)"
    }
}
