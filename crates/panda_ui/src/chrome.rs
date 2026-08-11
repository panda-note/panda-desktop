//! Cached chrome panes so Editor cursor motion does not rebuild the whole shell.
//!
//! GPUI marks every ancestor view dirty when a child notifies. AppShell is the
//! window root, so Vim j/k would otherwise re-render the memo list, nav, and
//! format toolbar on every motion. These panes are separate entities embedded
//! with [`Entity::cached`]; they only refresh when AppShell itself notifies
//! (data/UI changes), not when the editor is dirtied via the ancestor walk.

use gpui::{
    Context, Entity, IntoElement, Render, StyleRefinement, Subscription, WeakEntity, Window, div,
    prelude::*,
};

use crate::shell::AppShell;

pub(crate) struct SideChrome {
    shell: WeakEntity<AppShell>,
    _sub: Subscription,
}

pub(crate) struct EditorChrome {
    shell: WeakEntity<AppShell>,
    _sub: Subscription,
}

impl SideChrome {
    pub(crate) fn new(shell: Entity<AppShell>, cx: &mut Context<Self>) -> Self {
        let weak = shell.downgrade();
        let _sub = cx.observe(&shell, |_, _, cx| cx.notify());
        Self { shell: weak, _sub }
    }
}

impl EditorChrome {
    pub(crate) fn new(shell: Entity<AppShell>, cx: &mut Context<Self>) -> Self {
        let weak = shell.downgrade();
        let _sub = cx.observe(&shell, |_, _, cx| cx.notify());
        Self { shell: weak, _sub }
    }
}

impl Render for SideChrome {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(shell) = self.shell.upgrade() else {
            return div().into_any_element();
        };
        shell.update(cx, |shell, cx| shell.render_side_chrome(window, cx))
    }
}

impl Render for EditorChrome {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(shell) = self.shell.upgrade() else {
            return div().into_any_element();
        };
        shell.update(cx, |shell, cx| shell.render_editor_pane(window, cx))
    }
}

pub(crate) fn cached_flex_child_style() -> StyleRefinement {
    StyleRefinement::default()
        .flex_1()
        .h_full()
        .min_w_0()
        .min_h_0()
}
