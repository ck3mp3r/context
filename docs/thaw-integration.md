# Thaw UI Integration

## Overview

Thaw UI (v0.4) has been integrated into the frontend using a **hybrid approach**:
- **Thaw** for interactive components (forms, modals, complex UI)
- **Tailwind CSS** for layout, spacing, and custom components
- **Custom components** where they already work well (like swim lanes)

## Installation

```toml
# Cargo.toml
[dependencies]
thaw = { version = "0.4", features = ["csr"], optional = true }
```

**Features**: Added to `frontend` feature flag, so it only compiles for WASM builds.

## Available Thaw Components for c5t

### 🎯 Critical for Phase 3 (CRUD Operations)

#### Form Components
- **Input** - Single-line text input
  - Used for: Task names, project titles, repo URLs
  - Supports: prefix/suffix, autofocus, validation, disabled state
  - Example: `<Input value=signal placeholder="Task name"/>`

- **Textarea** - Multi-line text input
  - Used for: Note content, task descriptions
  - Supports: resize options, sizing, disabled state
  - Example: `<Textarea value=signal placeholder="Description"/>`

- **Field** - Form field wrapper
  - Provides: labels, validation messages, layout
  - Used for: Wrapping inputs with labels
  - Example: `<Field label="Task Name"><Input /></Field>`

#### Modal/Drawer Components
- **OverlayDrawer** - Slide-in panel
  - Used for: Create/edit forms (better than modals for complex forms)
  - Features: Modal/non-modal modes, backdrop, header/body/footer
  - Example:
    ```rust
    let open = RwSignal::new(false);
    
    view! {
        <Button on_click=move |_| open.set(true)>"Create Task"</Button>
        <OverlayDrawer open>
            <DrawerHeader>
                <DrawerHeaderTitle>"New Task"</DrawerHeaderTitle>
            </DrawerHeader>
            <DrawerBody>
                <Field label="Name"><Input /></Field>
            </DrawerBody>
        </OverlayDrawer>
    }
    ```

#### Selection Components
- **Select** - Basic dropdown
  - Used for: Single choice selections
  - Example: `<Select><option>"Option 1"</option></Select>`

- **Combobox** - Searchable select
  - Used for: Picking from long lists (task lists, projects)
  - Example:
    ```rust
    <Combobox selected_options placeholder="Select project">
        <ComboboxOption value="proj1" text="Project 1" />
    </Combobox>
    ```

- **MultiSelect** - Multiple selection
  - Used for: Selecting multiple items
  - Example: Multi-project assignment

- **TagPicker** - Tag selection UI
  - Used for: Managing tags on tasks/notes
  - Shows selected tags as chips

#### Action Components
- **Button** - Buttons with variants
  - Appearances: `Default`, `Primary`, `Subtle`
  - States: loading, disabled
  - Example: `<Button appearance=ButtonAppearance::Primary>"Save"</Button>`

### 📦 Already Using (Keep)
- **Accordion** - Custom implementation in `tasks.rs`
  - Works perfectly with swim lanes
  - No need to replace

### 🚫 Not Using from Thaw
- Spinner/Progress - Simple enough with Tailwind
- Card - Using Tailwind divs
- Alert/Toast - Can build simple ones with Tailwind if needed

## Usage Patterns

### Pattern 1: Form in Drawer

```rust
use thaw::*;

#[component]
fn CreateTaskDrawer(show: RwSignal<bool>) -> impl IntoView {
    let task_name = RwSignal::new(String::new());
    let task_content = RwSignal::new(String::new());
    
    let on_save = move |_| {
        // Call API to create task
        show.set(false);
    };
    
    view! {
        <OverlayDrawer open=show>
            <DrawerHeader>
                <DrawerHeaderTitle>"Create Task"</DrawerHeaderTitle>
            </DrawerHeader>
            <DrawerBody>
                <div class="space-y-4"> // Tailwind layout!
                    <Field label="Task Name">
                        <Input value=task_name placeholder="Enter task name"/>
                    </Field>
                    <Field label="Description">
                        <Textarea value=task_content placeholder="Task details..."/>
                    </Field>
                </div>
            </DrawerBody>
            <DrawerFooter>
                <div class="flex gap-2 justify-end"> // Tailwind layout!
                    <Button on_click=move |_| show.set(false)>"Cancel"</Button>
                    <Button appearance=ButtonAppearance::Primary on_click=on_save>
                        "Save"
                    </Button>
                </div>
            </DrawerFooter>
        </OverlayDrawer>
    }
}
```

### Pattern 2: Combobox for Selection

```rust
let selected_project = RwSignal::new(None::<String>);

view! {
    <Combobox selected_options=selected_project placeholder="Select a project">
        {move || {
            projects.get().iter().map(|proj| view! {
                <ComboboxOption value=proj.id.clone() text=proj.title.clone()/>
            }).collect_view()
        }}
    </Combobox>
}
```

### Pattern 3: TagPicker for Tags

```rust
let selected_tags = RwSignal::new(vec![]);

view! {
    <TagPicker selected_options=selected_tags>
        <TagPickerControl slot>
            <TagPickerGroup>
                {move || {
                    selected_tags.get().into_iter().map(|tag| view! {
                        <Tag value=tag.clone()>{tag}</Tag>
                    }).collect_view()
                }}
            </TagPickerGroup>
            <TagPickerInput />
        </TagPickerControl>
        <TagPickerOption value="frontend" text="frontend"/>
        <TagPickerOption value="backend" text="backend"/>
    </TagPicker>
}
```

## Bundle Size Impact

| Component        | Before (Tailwind only) | After (+ Thaw) | Increase  |
|------------------|------------------------|----------------|-----------|
| CSS              | ~20KB gzipped          | ~70KB gzipped  | +50KB     |
| WASM (approx)    | ~500KB                 | ~550KB         | +50KB     |

**Worth it?** YES, if we use it for CRUD. Saves ~500+ lines of custom form code.

## When to Use What

| Use Case                     | Library   | Why                                |
|------------------------------|-----------|-------------------------------------|
| Layout (flex, grid, spacing) | Tailwind  | Utility classes are faster          |
| Custom components            | Tailwind  | Full control over design            |
| Form inputs                  | Thaw      | Validation, accessibility built-in  |
| Modals/Drawers               | Thaw      | Complex interaction logic           |
| Dropdowns/Select             | Thaw      | Positioning, keyboard nav           |
| Existing swim lanes          | Custom    | Already works perfectly             |
| Task cards                   | Tailwind  | Simple, static display              |

## Next Steps for Phase 3 (CRUD)

### 1. Create Task Form
- Use `OverlayDrawer` for the form panel
- `Input` for task name
- `Textarea` for task content
- `Combobox` to select task list
- `TagPicker` for tags
- `Button` for save/cancel

### 2. Edit Task Form
- Same as create, but prefill values
- Load existing task data into signals

### 3. Delete Confirmation
- Simple Tailwind modal OR
- Use `OverlayDrawer` with warning styling

### 4. Create Note Form
- `Input` for title
- `Textarea` for content (consider markdown preview)
- `TagPicker` for tags
- `MultiSelect` for linking to projects

### 5. Quick Actions
- Add "New Task" button on Tasks page → opens drawer
- Add "New Note" button on Notes page → opens drawer
- Add "Edit" button on task cards → opens drawer with prefilled data

## Testing

```bash
# Build check
cargo check --bin c5t-frontend --features frontend --no-default-features

# Dev server with hot reload
trunk serve --open

# Check bundle size
trunk build --release
ls -lh dist/
```

## Resources

- Thaw UI Docs: https://thawui.vercel.app
- Thaw GitHub: https://github.com/thaw-ui/thaw
- Version: 0.4.8 (compatible with Leptos 0.7)
