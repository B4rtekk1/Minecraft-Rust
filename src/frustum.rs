use cgmath::{Matrix4, Vector3, Vector4};

#[derive(Clone, Copy)]
pub struct AABB {
    pub min: Vector3<f32>,
    pub max: Vector3<f32>,
}

impl AABB {
    pub fn new(min: Vector3<f32>, max: Vector3<f32>) -> Self {
        AABB { min, max }
    }

    pub fn is_visible(&self, frustum_planes: &[Vector4<f32>; 6]) -> bool {
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
            let p = Vector3::new(
                if plane.x > 0.0 {
                    expanded_max.x
                } else {
                    expanded_min.x
                },
                if plane.y > 0.0 {
                    expanded_max.y
                } else {
                    expanded_min.y
                },
                if plane.z > 0.0 {
                    expanded_max.z
                } else {
                    expanded_min.z
                },
            );
            if plane.x * p.x + plane.y * p.y + plane.z * p.z + plane.w < 0.0 {
                return false;
            }
        }
        true
    }
}

pub fn extract_frustum_planes(view_proj: &Matrix4<f32>) -> [Vector4<f32>; 6] {
    let m = view_proj;
    let mut planes = [
        // Left
        Vector4::new(
            m[0][3] + m[0][0],
            m[1][3] + m[1][0],
            m[2][3] + m[2][0],
            m[3][3] + m[3][0],
        ),
        // Right
        Vector4::new(
            m[0][3] - m[0][0],
            m[1][3] - m[1][0],
            m[2][3] - m[2][0],
            m[3][3] - m[3][0],
        ),
        // Bottom
        Vector4::new(
            m[0][3] + m[0][1],
            m[1][3] + m[1][1],
            m[2][3] + m[2][1],
            m[3][3] + m[3][1],
        ),
        // Top
        Vector4::new(
            m[0][3] - m[0][1],
            m[1][3] - m[1][1],
            m[2][3] - m[2][1],
            m[3][3] - m[3][1],
        ),
        // Near (WGPU depth is [0, 1])
        Vector4::new(m[0][2], m[1][2], m[2][2], m[3][2]),
        // Far
        Vector4::new(
            m[0][3] - m[0][2],
            m[1][3] - m[1][2],
            m[2][3] - m[2][2],
            m[3][3] - m[3][2],
        ),
    ];

    // Normalize planes so that distances are in world units
    for plane in &mut planes {
        let length = (plane.x * plane.x + plane.y * plane.y + plane.z * plane.z).sqrt();
        plane.x /= length;
        plane.y /= length;
        plane.z /= length;
        plane.w /= length;
    }

    planes
}
