use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use crate::graph::{Edge as GraphEdge, Graph};
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

fn cross_2d(vector_1: Vector2<f32>, vector_2: Vector2<f32>) -> f32 {
    vector_1.x * vector_2.y - vector_1.y * vector_2.x
}

fn on_line(point: Point2<f32>, line_start: Point2<f32>, line_direction: Vector2<f32>) -> bool {
    let start_point_line = point - line_start;
    let cross = cross_2d(start_point_line, line_direction);
    let dot_product = start_point_line.dot(&line_direction);
    let line_distance_squared = line_direction.magnitude_squared();
    cross == 0.0 && dot_product >= 0.0 && dot_product <= line_distance_squared
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

    fn corners_and_edges(&self) -> (Vec<Point2<f32>>, Vec<GraphEdge>) {
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
                GraphEdge(
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
                            Some(GraphEdge(e.1, e.0))
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

                let within_triangle =
                    a_cross.signum() == b_cross.signum() && a_cross.signum() == c_cross.signum();
                let on_line =
                    on_line(point, a, a_b) || on_line(point, b, b_c) || on_line(point, c, c_a);

                within_triangle || on_line
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
                            Some(GraphEdge(corrected_point_index, node_index))
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

#[cfg(test)]
mod tests {
    use super::*;
    use amethyst::core::alga::linear::EuclideanSpace;
    use std::iter::FromIterator;
    use test_case::test_case;

    #[test]
    fn into_vertices_produces_conserves_number_of_triangles_specified() {
        let map = Map {
            islands: vec![
                vec![
                    Point2::new(50, 100),
                    Point2::new(100, 125),
                    Point2::new(100, 75),
                ],
                vec![
                    Point2::new(50, 0),
                    Point2::new(100, 25),
                    Point2::new(100, -25),
                    Point2::new(100, -25),
                    Point2::new(100, 25),
                    Point2::new(150, 0),
                ],
            ],
        };
        let vertices = map.into_vertices();
        assert_eq!(9, vertices.len(), "number of vertices");
    }

    #[test_case(Point2::new(0.0, 0.0) => false ; "outside of land")]
    #[test_case(Point2::new(75.0, 110.0) => true ; "on smaller island")]
    #[test_case(Point2::new(110.0, 0.0) => true ; "on bigger island")]
    #[test_case(Point2::new(50.0, 0.0) => true ; "on corner")]
    #[test_case(Point2::new(52.0, 101.0) => true ; "on edge")]
    fn on_land_returns_true_if_point_on_any_triangle(point: Point2<f32>) -> bool {
        let map = Map {
            islands: vec![
                vec![
                    Point2::new(50, 100),
                    Point2::new(100, 125),
                    Point2::new(100, 75),
                ],
                vec![
                    Point2::new(50, 0),
                    Point2::new(100, 25),
                    Point2::new(100, -25),
                    Point2::new(100, -25),
                    Point2::new(100, 25),
                    Point2::new(150, 0),
                ],
            ],
        };
        map.on_land(point)
    }

    #[test_case(Point2::new(0.0, 0.0), Point2::new(50.0, 0.0) ; "point not on land")]
    #[test_case(Point2::new(65.0, 5.0), Point2::new(63.5, 7.89) ; "point on land")]
    #[test_case(Point2::new(98.0, 1.0), Point2::new(88.0, 20.0) ; "point near a middle edge")]
    fn closest_point_on_edge_selects_point_on_an_outer_edge_which_is_closest(
        point: Point2<f32>,
        unadjusted_point_on_edge: Point2<f32>,
    ) {
        let map = Map {
            islands: vec![vec![
                Point2::new(50, 0),
                Point2::new(100, 25),
                Point2::new(100, -25),
                Point2::new(100, -25),
                Point2::new(100, 25),
                Point2::new(150, 0),
            ]],
        };
        let closest_point = map.closest_point_on_edge(point);
        assert!(
            closest_point.distance(&unadjusted_point_on_edge) < 1.0,
            format!("Adjusted corner is close enough {:?}", closest_point)
        );
    }

    #[test_case(Point2::new(0.0, 0.0), Vector2::new(40.0, 1.0), true =>  None; "line outside of land and strict")]
    #[test_case(Point2::new(0.0, 0.0), Vector2::new(40.0, 1.0), false => Some(Point2::new(52.63158, 1.3157893)) ; "line outside of land but passing through and not strict")]
    #[test_case(Point2::new(0.0, 0.0), Vector2::new(0.0, 10.0), false => None ; "line outside of land not passing through and not strict")]
    #[test_case(Point2::new(0.0, 0.0), Vector2::new(40.0, 1.0), false => Some(Point2::new(52.63158, 1.3157893)) ; "line outside of land and not strict")]
    #[test_case(Point2::new(60.0, 2.5), Vector2::new(100.0, 0.0), true => Some(Point2::new(145.0, 2.5)) ; "starting point inside land, closest point behind and strict")]
    #[test_case(Point2::new(60.0, 3.0), Vector2::new(100.0, 0.0), false => Some(Point2::new(56.0, 3.0)) ; "starting point inside land, closest point behind and not strict")]
    fn closest_point_of_line_on_edge_finds_point_on_outer_edge_if_it_exists(
        starting_point: Point2<f32>,
        line_direction: Vector2<f32>,
        strict: bool,
    ) -> Option<Point2<f32>> {
        let map = Map {
            islands: vec![vec![
                Point2::new(50, 0),
                Point2::new(100, 25),
                Point2::new(100, -25),
                Point2::new(100, -25),
                Point2::new(100, 25),
                Point2::new(150, 0),
            ]],
        };
        map.closest_point_of_line_on_edge(starting_point, line_direction, strict)
    }

    #[test]
    fn nodes_and_edges_connected_does_not_returns_edges_for_points_that_cannot_directly_connect() {
        let map = Map {
            islands: vec![vec![
                Point2::new(50, 0),
                Point2::new(100, 25),
                Point2::new(100, -25),
            ]],
        };

        let graph =
            map.nodes_and_edges_connected(vec![Point2::new(0.0, 0.0), Point2::new(120.0, 0.0)]);
        let edges = HashSet::<GraphEdge>::from_iter(graph.edges.into_iter());

        assert!(
            !edges.contains(&GraphEdge(3, 4)) && !edges.contains(&GraphEdge(4, 3)),
            "Edges does not contain edge from start point to end point"
        );

        let number_connected_to_end_point = edges
            .iter()
            .filter(|edge| edge.0 == 4 || edge.1 == 4)
            .count();
        assert_eq!(
            2, number_connected_to_end_point,
            "Number of nodes end point is connected to"
        );
    }

    #[test]
    fn nodes_and_edges_connected_returns_edges_for_every_point_that_connects_to_a_corner() {
        let map = Map {
            islands: vec![vec![
                Point2::new(50, 0),
                Point2::new(100, 25),
                Point2::new(100, -25),
            ]],
        };

        let graph =
            map.nodes_and_edges_connected(vec![Point2::new(0.0, 0.0), Point2::new(120.0, 0.0)]);
        let edges = HashSet::<GraphEdge>::from_iter(graph.edges.into_iter());

        let number_connected_to_end_point = edges
            .iter()
            .filter(|edge| edge.0 == 3 || edge.1 == 3)
            .count();
        assert_eq!(
            3, number_connected_to_end_point,
            "Number of nodes start point is connected to"
        );

        let number_connected_to_end_point = edges
            .iter()
            .filter(|edge| edge.0 == 4 || edge.1 == 4)
            .count();
        assert_eq!(
            2, number_connected_to_end_point,
            "Number of nodes end point is connected to"
        );
    }

    #[test]
    fn nodes_and_edges_connected_adds_all_new_points_to_graph() {
        let map = Map {
            islands: vec![vec![
                Point2::new(50, 0),
                Point2::new(100, 25),
                Point2::new(100, -25),
            ]],
        };

        let graph = map.nodes_and_edges_connected(vec![
            Point2::new(0.0, 0.0),
            Point2::new(120.0, 0.0),
            Point2::new(150.0, 0.0),
        ]);
        assert_eq!(6, graph.nodes.len(), "Graph nodes");
    }

    #[test]
    fn nodes_and_edges_connected_only_includes_corners_and_outer_edges_of_land() {
        let map = Map {
            islands: vec![vec![
                Point2::new(50, 0),
                Point2::new(100, 25),
                Point2::new(100, -25),
                Point2::new(100, -25),
                Point2::new(100, 25),
                Point2::new(150, 0),
            ]],
        };

        let graph = map.nodes_and_edges_connected(vec![]);
        assert_eq!(4, graph.nodes.len(), "Graph nodes");
        assert_eq!(4, graph.edges.len(), "Graph edges");
    }
}
