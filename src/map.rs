use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use crate::graph::Graph;
use amethyst::{
    core::math::{distance, Point2, Vector2},
    renderer::rendy::mesh::Position,
};

#[derive(Default, Debug, Deserialize)]
pub struct Map {
    pub islands: Vec<Vec<Point2<i32>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Edge(Point2<i32>, Point2<i32>);

fn to_f32(point: Point2<i32>) -> Point2<f32> {
    Point2::new(point.x as f32, point.y as f32)
}

fn cross_2d(point_1: Vector2<f32>, point_2: Vector2<f32>) -> f32 {
    point_1.x * point_2.y - point_1.y * point_2.x
}

impl Map {
    pub fn into_vertices(&self) -> Vec<Position> {
        self.islands
            .iter()
            .flat_map(|island| {
                island
                    .iter()
                    .map(|&p| Position([p.x as f32, p.y as f32, 0.0]))
                    .clone()
            })
            .collect::<Vec<_>>()
    }

    fn outer_edges(&self) -> Vec<Edge> {
        self.islands
            .iter()
            .flat_map(|island| {
                let island_edges = island.chunks(3).flat_map(|triangle| {
                    vec![
                        Edge(triangle[0], triangle[1]),
                        Edge(triangle[1], triangle[2]),
                        Edge(triangle[2], triangle[0]),
                    ]
                });

                let mut island_outer_edges = HashMap::<Edge, usize>::new();
                for island_edge in island_edges {
                    let reverse_island_edge = Edge(island_edge.1, island_edge.0);

                    // Check both orders of points
                    let edge_count = island_outer_edges.get(&island_edge).map_or(0, |v| *v);
                    let reverse_edge_count = island_outer_edges
                        .get(&reverse_island_edge)
                        .map_or(0, |v| *v);

                    if edge_count > 0 {
                        island_outer_edges.insert(island_edge, edge_count + 1);
                    } else if reverse_edge_count > 0 {
                        island_outer_edges.insert(reverse_island_edge, reverse_edge_count + 1);
                    } else {
                        island_outer_edges.insert(island_edge, 1);
                    }
                }
                island_outer_edges
                    .iter()
                    .filter(|(_, &count)| count == 1)
                    .map(|(edge, _)| *edge)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<Edge>>()
    }

    fn corners_and_edges(&self) -> (Vec<Point2<f32>>, Vec<crate::graph::Edge>) {
        let outer_edges = self.outer_edges();

        let mut corners = HashSet::new();

        for outer_edge in &outer_edges {
            corners.insert(outer_edge.0);
            corners.insert(outer_edge.1);
        }

        let corners = corners.iter().map(|&c| c).collect::<Vec<_>>();
        let edges = outer_edges
            .iter()
            .map(|edge| {
                crate::graph::Edge(
                    corners.iter().position(|&c| c == edge.0).unwrap(),
                    corners.iter().position(|&c| c == edge.1).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let corners = corners.iter().map(|&c| to_f32(c)).collect::<Vec<_>>();

        let adjusted_corners = corners
            .iter()
            .enumerate()
            .map(|(corner_index, &c)| {
                let corner_edges = edges
                    .iter()
                    .filter_map(|e| {
                        if e.0 == corner_index {
                            Some(*e)
                        } else if e.1 == corner_index {
                            Some(crate::graph::Edge(e.1, e.0))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                let mut adjusted_corner = c;

                for edge in corner_edges {
                    adjusted_corner += (corners[edge.0] - corners[edge.1]).normalize();
                }
                adjusted_corner
            })
            .collect();

        (adjusted_corners, edges)
    }

    pub fn on_land(&self, point: Point2<f32>) -> bool {
        self.islands.iter().any(|island| {
            island.chunks(3).any(|triangle| {
                let a = to_f32(triangle[0]);
                let b = to_f32(triangle[1]);
                let c = to_f32(triangle[2]);
                let a_b = b - a;
                let b_c = c - b;
                let c_a = a - c;
                let a_p = point - a;
                let b_p = point - b;
                let c_p = point - c;

                let a_cross = cross_2d(a_b, a_p);
                let b_cross = cross_2d(b_c, b_p);
                let c_cross = cross_2d(c_a, c_p);

                a_cross.signum() == b_cross.signum() && a_cross.signum() == c_cross.signum()
            })
        })
    }

    pub fn closest_point_on_edge(&self, point: Point2<f32>) -> Point2<f32> {
        let outer_edges = self.outer_edges();
        let mut closest_point = (Point2::<f32>::origin(), f32::MAX);

        for outer_edge in outer_edges {
            let edge = (to_f32(outer_edge.0), to_f32(outer_edge.1));
            let distance_squared = distance(&edge.0, &edge.1).powf(2.0);

            let t = (point - edge.0).dot(&(edge.1 - edge.0)) / distance_squared;
            let clamped_t = if t > 1.0 {
                1.0
            } else if t < 0.0 {
                0.0
            } else {
                t
            };

            let closest_point_for_edge = edge.0 + clamped_t * (&(edge.1 - edge.0));
            let distance = distance(&closest_point_for_edge, &point);
            if distance < closest_point.1 {
                let direction = (edge.1 - edge.0).normalize();
                let perpendicular_direction = Vector2::new(-direction.y, direction.x);
                let adjusted_closest_point_for_edge =
                    closest_point_for_edge + perpendicular_direction;
                let adjusted_closest_point_for_edge =
                    if self.on_land(adjusted_closest_point_for_edge) {
                        closest_point_for_edge - perpendicular_direction
                    } else {
                        adjusted_closest_point_for_edge
                    };

                closest_point = (adjusted_closest_point_for_edge, distance);
            }
        }

        closest_point.0
    }

    pub fn closest_point_of_line_on_edge(
        &self,
        starting_point: Point2<f32>,
        line_direction: Vector2<f32>,
        strict: bool,
    ) -> Option<Point2<f32>> {
        let outer_edges = self.outer_edges();
        let mut closest_point = (None, f32::MAX);

        for outer_edge in outer_edges {
            let edge_direction = to_f32(outer_edge.1) - to_f32(outer_edge.0);
            let edge_starting_point = to_f32(outer_edge.0);

            let cross_of_directions = cross_2d(line_direction, edge_direction);

            let mut possible_point_on_edge = None;

            if cross_of_directions == 0.0 {
                // parallel
                if cross_2d(edge_starting_point - starting_point, line_direction) == 0.0 {
                    // colinear
                    let t_0 = (edge_starting_point - starting_point).dot(&line_direction)
                        / (line_direction.dot(&line_direction));

                    let t_1 = t_0
                        + edge_direction.dot(&line_direction)
                            / (line_direction.dot(&line_direction));
                    if t_1 <= 1.0 && t_1 >= 0.0 {
                        let point_on_edge = Some(edge_starting_point + t_1 * edge_direction);
                        if strict {
                            if t_0 >= 0.0 && t_0 <= 1.0 {
                                possible_point_on_edge = point_on_edge;
                            }
                        } else {
                            possible_point_on_edge = point_on_edge;
                        }
                    }
                }
            } else {
                // not parallel
                let t = cross_2d(
                    edge_starting_point - starting_point,
                    edge_direction / cross_of_directions,
                );
                let u = cross_2d(
                    edge_starting_point - starting_point,
                    line_direction / cross_of_directions,
                );

                if u >= 0.0 && u <= 1.0 {
                    let point_on_edge = Some(edge_starting_point + u * edge_direction);
                    if strict {
                        if t >= 0.0 && t <= 1.0 {
                            possible_point_on_edge = point_on_edge;
                        }
                    } else {
                        possible_point_on_edge = point_on_edge;
                    }
                }
            }

            if let Some(point_on_edge) = possible_point_on_edge {
                let distance = distance(&point_on_edge, &starting_point);
                if distance < closest_point.1 {
                    closest_point = (Some(point_on_edge), distance);
                }
            }
        }
        closest_point.0
    }

    pub fn nodes_and_edges_connected(&self, points: Vec<Point2<f32>>) -> Graph {
        let (corners, mut edges) = self.corners_and_edges();
        let nodes = corners
            .iter()
            .chain(points.iter())
            .map(|&p| p)
            .collect::<Vec<Point2<f32>>>();

        let corners_count = corners.len();
        let mut new_edges = points
            .iter()
            .enumerate()
            .flat_map(|(point_index, &point)| {
                let corrected_point_index = point_index + corners_count;
                nodes
                    .iter()
                    .enumerate()
                    .filter_map(move |(node_index, &node)| {
                        let closest_point =
                            self.closest_point_of_line_on_edge(point, node - point, true);
                        let edge = if closest_point.is_none() && corrected_point_index != node_index
                        {
                            Some(crate::graph::Edge(corrected_point_index, node_index))
                        } else {
                            None
                        };
                        edge
                    })
            })
            .collect::<Vec<_>>();
        edges.append(&mut new_edges);
        Graph {
            nodes: nodes,
            edges: edges,
        }
    }
}
