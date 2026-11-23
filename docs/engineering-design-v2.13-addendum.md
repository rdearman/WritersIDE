# Engineering Design v2.13 Addendum — New Fiction and Non-fiction Projects

This addendum extends the terminal project-creation workflow while retaining the existing `BookType`, portable project layout, two-pane file browser, project-switch service and manuscript storage model.

## Project creation contract

`File → New Project` opens the shared project browser. Before creation, the author must explicitly choose **Fiction** or **Non-fiction** using the visible `F2` book-type selector. The browser displays the current choice, parent path and project name. `F5` cannot create a project until a type has been selected. Existing destination refusal and cancellation behavior remain unchanged.

The selected type is passed to `WriterApp::create_project_with_type`; the project identity and type are then verified by the existing project-opening path. No chapter, scene or prose is synthesized by project creation.

After successful creation, fiction opens in **Scene Cards**, while non-fiction opens in **Outline**. Scene Cards advertises `Insert: New Scene`; the Insert key dispatches the existing `scene.create` command through the shared registry, making repeated scene creation possible without reopening a menu. Non-fiction remains scene-free and can begin with chapter prose through its existing Start Writing path.

## Regression coverage

Terminal tests verify explicit type selection, refusal to create before choosing a type, type persistence, cancellation and existing-directory protection, Fiction → Scene Cards, Non-fiction → Outline, and creation of 60 uniquely identified scenes through the card view's Insert action. The 80×24 file-browser rendering test asserts the type choice and create/navigation controls remain discoverable.

This milestone does not claim that the chapter prose workflow, nested-section outline or the three complete human UAT journeys are finished. Those remain part of the queued author-workflow milestone.
