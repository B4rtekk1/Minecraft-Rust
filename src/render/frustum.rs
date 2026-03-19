use cgmath::{Matrix4, Vector3, Vector4};

/// An axis-aligned bounding box defined by its minimum and maximum corners.
///
/// Used for frustum culling to quickly reject geometry that lies entirely
/// outside the view frustum without inspecting individual vertices.
#[derive(Clone, Copy)]
pub struct AABB {
    /// World-space corner with the smallest x, y, and z coordinates.
    pub min: Vector3<f32>,
    /// World-space corner with the largest x, y, and z coordinates.
    pub max: Vector3<f32>,
}

impl AABB {
    /// Creates a new `AABB` from explicit minimum and maximum corners.
    pub fn new(min: Vector3<f32>, max: Vector3<f32>) -> Self {
        AABB { min, max }
    }

    /// Tests whether this AABB intersects or lies inside the given view frustum.
    ///
    /// The AABB is expanded by a small `margin` on all sides before testing to
    /// avoid popping artifacts at frustum edges caused by floating-point
    /// imprecision or geometry that slightly overhangs its bounding box.
    ///
    /// The test uses the *positive-vertex* method: for each frustum plane the
    /// corner of the (expanded) box that is furthest along the plane normal is
    /// chosen as the representative point.  If that point lies on the negative
    /// side of any plane the entire box is outside the frustum.
    ///
    /// # Arguments
    /// * `frustum_planes` – Six normalized frustum planes in world space
    ///   (left, right, bottom, top, near, far), each stored as `(nx, ny, nz, d)`
    ///   where the plane equation is `n·p + d ≥ 0` for points inside.
    ///
    /// # Returns
    /// `true` if the AABB is potentially visible; `false` if it is definitely
    /// outside the frustum and can be safely culled.
    pub fn is_visible(&self, frustum_planes: &[Vector4<f32>; 6]) -> bool {
        // A small world-space margin to guard against edge-case popping.
        let margin = 2.0;
        let expanded_min = Vector3::new(
            self.min.x - margin,
            self.min.y - margin,
            self.min.z - margin,
        );
        let expanded_max = Vector3::new(
            self.max.x + margin,
            self.max.y + margin,
            self.max.z + margin,
        );

        for plane in frustum_planes {
            // Select the AABB corner that is furthest in the direction of the
            // plane normal (the "positive vertex").  If even this most-positive
            // corner is behind the plane, the box is fully outside.
            let p = Vector3::new(
                if plane.x > 0.0 { expanded_max.x } else { expanded_min.x },
                if plane.y > 0.0 { expanded_max.y } else { expanded_min.y },
                if plane.z > 0.0 { expanded_max.z } else { expanded_min.z },
            );
            if plane.x * p.x + plane.y * p.y + plane.z * p.z + plane.w < 0.0 {
                return false;
            }
        }
        true
    }
}

/// Extracts and normalizes the six frustum planes from a combined view-projection matrix.
///
/// Planes are derived using Grib & Hartmann's method of combining rows of the
/// clip-space matrix.  After extraction each plane is divided by the magnitude
/// of its normal so that the `w` component represents the true signed distance
/// from the origin to the plane, enabling accurate distance comparisons.
///
/// The resulting planes follow the convention `n·p + d ≥ 0` for points on the
/// *inside* of the frustum, where `(nx, ny, nz)` is the inward-facing normal
/// and `d` is stored in the `w` component.
///
/// # Arguments
/// * `view_proj` – The combined view-projection matrix (column-major, as cgmath
///   stores it).  The matrix must use a left-handed clip space (z ∈ [0, 1]),
///   which matches wgpu / Vulkan conventions.
///
/// # Returns
/// An array of six normalized planes in the order:
/// `[left, right, bottom, top, near, far]`.
pub fn extract_frustum_planes(view_proj: &Matrix4<f32>) -> [Vector4<f32>; 6] {
    // cgmath stores matrices column-major: m[col][row].
    let m = view_proj;
    let mut planes = [
        // Left:   row3 + row0
        Vector4::new(
            m[0][3] + m[0][0],
            m[1][3] + m[1][0],
            m[2][3] + m[2][0],
            m[3][3] + m[3][0],
        ),
        // Right:  row3 - row0
        Vector4::new(
            m[0][3] - m[0][0],
            m[1][3] - m[1][0],
            m[2][3] - m[2][0],
            m[3][3] - m[3][0],
        ),
        // Bottom: row3 + row1
        Vector4::new(
            m[0][3] + m[0][1],
            m[1][3] + m[1][1],
            m[2][3] + m[2][1],
            m[3][3] + m[3][1],
        ),
        // Top:    row3 - row1
        Vector4::new(
            m[0][3] - m[0][1],
            m[1][3] - m[1][1],
            m[2][3] - m[2][1],
            m[3][3] - m[3][1],
        ),
        // Near:   row2  (z ∈ [0,1] clip space — no row3 addition needed)
        Vector4::new(m[0][2], m[1][2], m[2][2], m[3][2]),
        // Far:    row3 - row2
        Vector4::new(
            m[0][3] - m[0][2],
            m[1][3] - m[1][2],
            m[2][3] - m[2][2],
            m[3][3] - m[3][2],
        ),
    ];

    // Normalize each plane so its normal has unit length.  This makes the `w`
    // component a true signed distance and allows mixing plane tests with
    // distance-based comparisons (e.g. LOD selection).
    for plane in &mut planes {
        let length = (plane.x * plane.x + plane.y * plane.y + plane.z * plane.z).sqrt();
        plane.x /= length;
        plane.y /= length;
        plane.z /= length;
        plane.w /= length;
    }

    planes
}