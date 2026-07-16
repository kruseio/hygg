#![cfg(any(feature = "pdf-rendering", test))]

use crate::stream::types::PdfRegion;
use crate::stream::types::VisualTextRow;
use crate::stream::vector_geom::{
  has_native_text_inside_region, has_nearby_figure_caption,
};

pub(crate) fn detect_vector_diagram_regions(
  paths: &[pdf_oxide::elements::PathContent],
  page_left: f32,
  page_top: f32,
  page_width: f32,
  page_height: f32,
  native_rows: &[VisualTextRow],
  allow_missing_native_text: bool,
) -> Vec<PdfRegion> {
  let mut clusters: Vec<VectorPathCluster> = Vec::new();

  // Clustering is quadratic in the worst case: every accepted path is compared
  // against every cluster so far, and a path far from all of them starts
  // another cluster. Real diagrams converge — tens of primitives collapsing
  // into one or two regions — but a page is free to carry a hundred thousand
  // scattered table primitives, and then nothing converges and the scan below
  // is O(n^2), before any of the cheap "is this even a diagram" checks get to
  // run. The densest page in the test corpus is under 4k paths.
  const MAX_VECTOR_PATHS: usize = 4096;

  for path in paths.iter().take(MAX_VECTOR_PATHS) {
    let bbox = path.bbox;
    if !path.is_table_primitive()
      || !bbox.x.is_finite()
      || !bbox.y.is_finite()
      || !bbox.width.is_finite()
      || !bbox.height.is_finite()
      || (bbox.width <= 0.0 && bbox.height <= 0.0)
      || bbox.width > page_width * 0.95
      || bbox.height > page_height * 0.95
    {
      continue;
    }

    let bounds = VectorPathBounds {
      left: bbox.left(),
      bottom: bbox.top(),
      right: bbox.right(),
      top: bbox.bottom(),
    };
    add_vector_path_to_clusters(&mut clusters, bounds);
  }

  let page_right = page_left + page_width;
  let page_bottom = page_top + page_height;
  clusters
    .into_iter()
    .filter(|cluster| cluster.count >= 3)
    .filter_map(|cluster| {
      cluster.region_with_padding(page_left, page_top, page_right, page_bottom)
    })
    .filter(|region| region.width >= 24.0 && region.height >= 24.0)
    .filter(|region| {
      should_render_vector_diagram_region(
        *region,
        native_rows,
        allow_missing_native_text,
      )
    })
    .collect()
}

#[derive(Clone, Copy, Debug)]
struct VectorPathBounds {
  left: f32,
  bottom: f32,
  right: f32,
  top: f32,
}

#[derive(Clone, Copy, Debug)]
struct VectorPathCluster {
  count: usize,
  left: f32,
  bottom: f32,
  right: f32,
  top: f32,
}

impl VectorPathCluster {
  fn new(bounds: VectorPathBounds) -> Self {
    Self {
      count: 1,
      left: bounds.left,
      bottom: bounds.bottom,
      right: bounds.right,
      top: bounds.top,
    }
  }

  fn is_near(&self, bounds: VectorPathBounds) -> bool {
    const CLUSTER_TOLERANCE: f32 = 48.0;
    bounds.left <= self.right + CLUSTER_TOLERANCE
      && bounds.right >= self.left - CLUSTER_TOLERANCE
      && bounds.bottom <= self.top + CLUSTER_TOLERANCE
      && bounds.top >= self.bottom - CLUSTER_TOLERANCE
  }

  fn merge_bounds(&mut self, bounds: VectorPathBounds) {
    self.count += 1;
    self.left = self.left.min(bounds.left);
    self.bottom = self.bottom.min(bounds.bottom);
    self.right = self.right.max(bounds.right);
    self.top = self.top.max(bounds.top);
  }

  fn merge_cluster(&mut self, other: Self) {
    self.count += other.count;
    self.left = self.left.min(other.left);
    self.bottom = self.bottom.min(other.bottom);
    self.right = self.right.max(other.right);
    self.top = self.top.max(other.top);
  }

  fn region_with_padding(
    &self,
    page_left: f32,
    page_top: f32,
    page_right: f32,
    page_bottom: f32,
  ) -> Option<PdfRegion> {
    if !self.left.is_finite() || !self.bottom.is_finite() {
      return None;
    }
    let pad = 4.0;
    let padded_left = (self.left - pad).max(page_left);
    let padded_bottom = (self.bottom - pad).max(page_top);
    let padded_right = (self.right + pad).min(page_right);
    let padded_top = (self.top + pad).min(page_bottom);
    Some(PdfRegion {
      left: padded_left,
      bottom: padded_bottom,
      width: (padded_right - padded_left).max(0.0),
      height: (padded_top - padded_bottom).max(0.0),
    })
  }
}

fn add_vector_path_to_clusters(
  clusters: &mut Vec<VectorPathCluster>,
  bounds: VectorPathBounds,
) {
  let Some(mut cluster_idx) =
    clusters.iter().position(|cluster| cluster.is_near(bounds))
  else {
    clusters.push(VectorPathCluster::new(bounds));
    return;
  };

  clusters[cluster_idx].merge_bounds(bounds);
  let mut idx = 0;
  while idx < clusters.len() {
    if idx != cluster_idx
      && clusters[cluster_idx].is_near(VectorPathBounds {
        left: clusters[idx].left,
        bottom: clusters[idx].bottom,
        right: clusters[idx].right,
        top: clusters[idx].top,
      })
    {
      let other = clusters.remove(idx);
      if idx < cluster_idx {
        cluster_idx -= 1;
      }
      clusters[cluster_idx].merge_cluster(other);
    } else {
      idx += 1;
    }
  }
}

fn should_render_vector_diagram_region(
  region: PdfRegion,
  native_rows: &[VisualTextRow],
  allow_missing_native_text: bool,
) -> bool {
  if !has_nearby_figure_caption(region, native_rows) {
    return false;
  }
  allow_missing_native_text
    || has_native_text_inside_region(region, native_rows)
}
