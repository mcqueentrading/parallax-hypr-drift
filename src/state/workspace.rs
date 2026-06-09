use smithay::desktop::Window;
use smithay::reexports::wayland_server::backend::ObjectId;
use smithay::utils::{Logical, Point, Rectangle, Size};
use std::collections::{HashMap, HashSet};

use super::DriftWm;
use driftwm::config::WorkspaceLayout;

pub type WorkspaceId = u8;
pub type DwindleNodeId = u64;

const WORKSPACE_WIDTH: i32 = 1920;
const WORKSPACE_HEIGHT: i32 = 1080;
const WORKSPACE_GAP: i32 = 280;

#[derive(Clone, Debug)]
pub struct WorkspaceState {
    pub rect: Rectangle<i32, Logical>,
    pub windows: HashSet<ObjectId>,
    pub tile_rects: HashMap<ObjectId, Rectangle<i32, Logical>>,
    pub dwindle_root: Option<DwindleNodeId>,
    pub dwindle_nodes: HashMap<DwindleNodeId, DwindleNode>,
    pub next_dwindle_node: DwindleNodeId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DwindleSplit {
    Vertical,
    Horizontal,
}

impl DwindleSplit {
    pub fn toggled(self) -> Self {
        match self {
            Self::Vertical => Self::Horizontal,
            Self::Horizontal => Self::Vertical,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DwindleNode {
    pub parent: Option<DwindleNodeId>,
    pub window: Option<ObjectId>,
    pub children: Option<(DwindleNodeId, DwindleNodeId)>,
    pub split: DwindleSplit,
    pub ratio: f32,
}

impl WorkspaceState {
    fn new(_id: WorkspaceId, rect: Rectangle<i32, Logical>) -> Self {
        Self {
            rect,
            windows: HashSet::new(),
            tile_rects: HashMap::new(),
            dwindle_root: None,
            dwindle_nodes: HashMap::new(),
            next_dwindle_node: 1,
        }
    }

    pub fn alloc_dwindle_node(&mut self, node: DwindleNode) -> DwindleNodeId {
        let id = self.next_dwindle_node;
        self.next_dwindle_node += 1;
        self.dwindle_nodes.insert(id, node);
        id
    }
}

pub fn default_workspaces(layout: WorkspaceLayout) -> HashMap<WorkspaceId, WorkspaceState> {
    // Internal DriftWM canvas coordinates are Y-down. The six workspaces are
    // laid out as a flat grid by default:
    //
    //   1   2   3
    //   4   5   6
    //
    // CubeNet is reserved for the later parallax projection:
    //
    //       4
    //   1   2   3   6
    //       5
    //
    // This deliberately does not use the older user-facing `go-to` Y-up
    // convention, which negates Y before moving the camera.
    let step_x = WORKSPACE_WIDTH + WORKSPACE_GAP;
    let step_y = WORKSPACE_HEIGHT + WORKSPACE_GAP;
    let origins = match layout {
        WorkspaceLayout::Grid => [
            (1, 0, 0),
            (2, step_x, 0),
            (3, step_x * 2, 0),
            (4, 0, step_y),
            (5, step_x, step_y),
            (6, step_x * 2, step_y),
        ],
        WorkspaceLayout::CubeNet => [
            (1, 0, step_y),
            (2, step_x, step_y),
            (3, step_x * 2, step_y),
            (4, step_x, 0),
            (5, step_x, step_y * 2),
            (6, step_x * 3, step_y),
        ],
    };

    origins
        .into_iter()
        .map(|(id, x, y)| {
            (
                id,
                WorkspaceState::new(
                    id,
                    Rectangle::new(
                        Point::from((x, y)),
                        Size::from((WORKSPACE_WIDTH, WORKSPACE_HEIGHT)),
                    ),
                ),
            )
        })
        .collect()
}

impl DriftWm {
    pub fn workspace_at_point(&self, point: Point<i32, Logical>) -> Option<WorkspaceId> {
        self.workspaces
            .iter()
            .find_map(|(&id, workspace)| workspace.rect.contains(point).then_some(id))
    }

    pub fn sync_active_workspace_from_pointer(&mut self) {
        let Some(pointer) = self.seat.get_pointer() else {
            return;
        };
        let point = pointer.current_location().to_i32_round::<i32>();
        let Some(output) = self.active_output() else {
            return;
        };
        let os = super::output_state(&output);
        let visible = driftwm::canvas::visible_canvas_rect(
            os.camera.to_i32_round(),
            super::output_logical_size(&output),
            os.zoom,
        );
        drop(os);
        if !visible.contains(point) {
            return;
        }
        if let Some(id) = self.workspace_at_point(point) {
            self.active_workspace = id;
        }
    }

    pub fn active_workspace_rect(&self) -> Rectangle<i32, Logical> {
        self.workspaces
            .get(&self.active_workspace)
            .or_else(|| self.workspaces.get(&1))
            .map(|workspace| workspace.rect)
            .unwrap_or_else(|| Rectangle::new(Point::from((0, 0)), Size::from((1920, 1080))))
    }

    pub fn workspace_for_window(&self, window: &Window) -> Option<WorkspaceId> {
        let loc = self.space.element_location(window)?;
        let size = window.geometry().size;
        let center = Point::<i32, Logical>::from((loc.x + size.w / 2, loc.y + size.h / 2));
        self.workspace_at_point(center)
    }

    pub fn assign_window_to_workspace(&mut self, id: ObjectId, workspace_id: WorkspaceId) {
        for workspace in self.workspaces.values_mut() {
            workspace.windows.remove(&id);
            workspace.tile_rects.remove(&id);
            crate::state::tile::remove_dwindle_window(workspace, &id);
        }
        if let Some(workspace) = self.workspaces.get_mut(&workspace_id) {
            workspace.windows.insert(id);
        }
    }

    pub fn activate_workspace(&mut self, id: WorkspaceId) {
        let Some(workspace) = self.workspaces.get(&id) else {
            tracing::warn!("requested missing workspace {id}");
            return;
        };
        self.active_workspace = id;

        let vc = self.usable_center_screen();
        let target_zoom = 1.0;
        let center = Point::<f64, Logical>::from((
            workspace.rect.loc.x as f64 + workspace.rect.size.w as f64 / 2.0,
            workspace.rect.loc.y as f64 + workspace.rect.size.h as f64 / 2.0,
        ));
        let target_camera =
            Point::from((center.x - vc.x / target_zoom, center.y - vc.y / target_zoom));
        self.set_overview_return(None);
        self.with_output_state(|os| {
            os.home_return = None;
        });
        self.set_zoom_animation_center(Some(center));
        self.set_camera_target(Some(target_camera));
        self.set_zoom_target(Some(target_zoom));
        crate::diagnostics::log(format!(
            "workspace:activate id={id} camera=({:.1},{:.1}) zoom={target_zoom:.3}",
            target_camera.x, target_camera.y
        ));
    }
}
