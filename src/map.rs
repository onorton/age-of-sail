use crate::graph::{Edge as GraphEdge, Graph};
use amethyst::{
    core::{
        alga::linear::EuclideanSpace,
        math::{distance, Point2, Vector2},
    },
    renderer::rendy::mesh::Position,
};
use itertools::Itertools;
use serde::Deserialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::iter::FromIterator;

const COORDINATE_MAX: f32 = 10000.000;

#[derive(Default, Debug, Deserialize)]
pub struct Map {
    pub islands: Vec<Vec<Point2<i32>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Edge(Point2<i32>, Point2<i32>);

impl Edge {
    fn intersects(&self, other: &Edge) -> bool {
        let self_direction = to_f32(self.1) - to_f32(self.0);
        let self_start = to_f32(self.0);

        let other_direction = to_f32(other.1) - to_f32(other.0);
        let other_start = to_f32(other.0);

        intersect(self_start, self_direction, other_start, other_direction)
    }

    fn order_by_y(&self) -> Edge {
        if self.0.y > self.1.y {
            Edge(self.0, self.1)
        } else {
            Edge(self.1, self.0)
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeF32(Point2<f32>, Point2<f32>);

impl EdgeF32 {
    fn intersects(&self, other: &EdgeF32) -> bool {
        let self_direction = self.1 - self.0;
        let self_start = self.0;

        let other_direction = other.1 - other.0;
        let other_start = other.0;

        intersect_forgiving(self_start, self_direction, other_start, other_direction).is_some()
    }

    fn order_by_y(&self) -> EdgeF32 {
        if self.0.y > self.1.y {
            EdgeF32(self.0, self.1)
        } else {
            EdgeF32(self.1, self.0)
        }
    }
}

fn to_f32(point: Point2<i32>) -> Point2<f32> {
    Point2::new(point.x as f32, point.y as f32)
}

fn to_i32(point: Point2<f32>) -> Point2<i32> {
    Point2::new(point.x.round() as i32, point.y.round() as i32)
}

fn intersect(
    a_start: Point2<f32>,
    a_direction: Vector2<f32>,
    b_start: Point2<f32>,
    b_direction: Vector2<f32>,
) -> bool {
    let cross_of_directions = cross_2d(a_direction, b_direction);
    if cross_of_directions == 0.0 {
        // parallel
        if cross_2d(b_start - a_start, a_direction) == 0.0 {
            // colinear
            let t_0 = (b_start - a_start).dot(&a_direction) / (a_direction.dot(&a_direction));
            let t_1 = t_0 + b_direction.dot(&a_direction) / (a_direction.dot(&a_direction));

            t_1 <= 1.0 && t_1 >= 0.0 && t_0 >= 0.0 && t_0 <= 1.0
        } else {
            false
        }
    } else {
        // not parallel
        if a_start == b_start
            || a_start == (b_start + b_direction)
            || (a_start + a_direction) == b_start
            || (a_start + a_direction) == (b_start + b_direction)
        {
            return false;
        }
        let t = cross_2d(b_start - a_start, b_direction / cross_of_directions);
        let u = cross_2d(b_start - a_start, a_direction / cross_of_directions);
        t >= 0.0 && t <= 1.0 && u >= 0.0 && u <= 1.0
    }
}

fn intersect_forgiving(
    a_start: Point2<f32>,
    a_direction: Vector2<f32>,
    b_start: Point2<f32>,
    b_direction: Vector2<f32>,
) -> Option<Point2<f32>> {
    let cross_of_directions = cross_2d(a_direction, b_direction);
    let epsilon = 0.01;
    if cross_of_directions == 0.0 {
        // parallel
        if cross_2d(b_start - a_start, a_direction) == 0.0 {
            // colinear
            let t_0 = (b_start - a_start).dot(&a_direction) / (a_direction.dot(&a_direction));
            let t_1 = t_0 + b_direction.dot(&a_direction) / (a_direction.dot(&a_direction));

            if (t_1 <= 1.0 + epsilon
                && t_1 >= 0.0 - epsilon
                && t_0 >= 0.0 - epsilon
                && t_0 <= 1.0 + epsilon)
            {
                Some(a_start + t_0 * a_direction)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        let t = cross_2d(b_start - a_start, b_direction / cross_of_directions);
        let u = cross_2d(b_start - a_start, a_direction / cross_of_directions);
        if t >= 0.0 - epsilon && t <= 1.0 + epsilon && u >= 0.0 - epsilon && u <= 1.0 + epsilon {
            Some(a_start + t * a_direction)
        } else {
            None
        }
    }
}

fn cross_2d(vector_1: Vector2<f32>, vector_2: Vector2<f32>) -> f32 {
    vector_1.x * vector_2.y - vector_1.y * vector_2.x
}

fn on_line(point: Point2<f32>, line_start: Point2<f32>, line_direction: Vector2<f32>) -> bool {
    if line_direction.magnitude() == 0.0 {
        return point == line_start;
    }

    let start_point_line = point - line_start;
    let cross = cross_2d(start_point_line, line_direction);
    let dot_product = start_point_line.dot(&line_direction);
    let line_distance_squared = line_direction.magnitude_squared();
    cross.abs() <= 0.02 && dot_product >= 0.0 && dot_product <= line_distance_squared
}

#[derive(Clone, Debug)]
enum QueryNode {
    X(EdgeF32, Box<QueryNode>, Box<QueryNode>),
    Y(f32, Box<QueryNode>, Box<QueryNode>),
    Trapezoid(usize),
}

impl QueryNode {
    fn next(&self, point: Point2<i32>) -> &QueryNode {
        match self {
            QueryNode::X(e, left, right) => {
                let dir = if e.0.y > e.1.y { e.0 - e.1 } else { e.1 - e.0 };
                let cross = cross_2d(to_f32(point) - Point2::new(0.0, 0.0), dir);

                if cross < 0.0 {
                    (&**right).next(point)
                } else {
                    (&**left).next(point)
                }
            }
            QueryNode::Y(y, above, below) => {
                if point.y as f32 >= *y {
                    (&**above).next(point)
                } else {
                    (&**below).next(point)
                }
            }
            other => other,
        }
    }

    fn insert_node(&mut self, original_trapezoid_index: usize, node: QueryNode) {
        match self {
            QueryNode::X(_, left, right) => {
                let node_cloned = node.clone();
                left.insert_node(original_trapezoid_index, node);
                right.insert_node(original_trapezoid_index, node_cloned);
            }
            QueryNode::Y(_, above, below) => {
                let node_cloned = node.clone();
                above.insert_node(original_trapezoid_index, node);
                below.insert_node(original_trapezoid_index, node_cloned);
            }
            QueryNode::Trapezoid(index) => {
                if *index == original_trapezoid_index {
                    *self = node;
                }
            }
        };
    }
}

#[derive(Debug)]
struct QueryStructure {
    root: QueryNode,
}

impl QueryStructure {
    fn query(&self, point: Point2<i32>) -> usize {
        if let QueryNode::Trapezoid(index) = self.root.next(point) {
            *index
        } else {
            0
        }
    }

    fn insert_y_node(
        &mut self,
        original_trapezoid_index: usize,
        trapezoid_above_index: usize,
        trapezoid_below_index: usize,
        y: f32,
    ) {
        let new_node = QueryNode::Y(
            y,
            Box::new(QueryNode::Trapezoid(trapezoid_above_index)),
            Box::new(QueryNode::Trapezoid(trapezoid_below_index)),
        );

        self.root.insert_node(original_trapezoid_index, new_node);
    }

    fn insert_x_node(
        &mut self,
        original_trapezoid_index: usize,
        trapezoid_left_index: usize,
        trapezoid_right_index: usize,
        segment: EdgeF32,
    ) {
        let new_node = QueryNode::X(
            segment,
            Box::new(QueryNode::Trapezoid(trapezoid_left_index)),
            Box::new(QueryNode::Trapezoid(trapezoid_right_index)),
        );

        self.root.insert_node(original_trapezoid_index, new_node);
    }
}

// None means trapezoid extends infinitely to - or + inf
#[derive(Clone, Debug)]
struct Trapezoid {
    left: EdgeF32,
    right: EdgeF32,
}

impl Trapezoid {
    fn split_vertically(&self, y: f32) -> (Trapezoid, Trapezoid) {
        let left_top_first = self.left.order_by_y();
        let right_top_first = self.right.order_by_y();

        let diff = left_top_first.1 - left_top_first.0;
        let proportion = (y - left_top_first.0.y) / diff.y;
        let left_mid = left_top_first.0 + proportion * diff;

        let diff = right_top_first.1 - right_top_first.0;
        let proportion = (y - right_top_first.0.y) / diff.y;
        let right_mid = right_top_first.0 + proportion * diff;

        let above_left = EdgeF32(left_top_first.0, left_mid);
        let above_right = EdgeF32(right_top_first.0, right_mid);
        let below_left = EdgeF32(left_mid, left_top_first.1);
        let below_right = EdgeF32(right_mid, right_top_first.1);
        (
            Trapezoid {
                left: above_left,
                right: above_right,
            },
            Trapezoid {
                left: below_left,
                right: below_right,
            },
        )
    }

    fn horizontal_edges(&self) -> [EdgeF32; 2] {
        let left_top_first = self.left.order_by_y();
        let right_top_first = self.right.order_by_y();

        [
            EdgeF32(left_top_first.0, right_top_first.0),
            EdgeF32(left_top_first.1, right_top_first.1),
        ]
    }

    fn vertices(&self) -> [Point2<f32>; 4] {
        let horizontal_edges = self.horizontal_edges();
        [
            horizontal_edges[0].0,
            horizontal_edges[0].1,
            horizontal_edges[1].1,
            horizontal_edges[1].0,
        ]
    }

    // If it intersects a trapezoid, guaranteed to intersect both horizontal segments due to
    // construction
    fn segment_intersects_horizontal_edges(&self, segment: EdgeF32) -> bool {
        let horizontal_edges = self.horizontal_edges();
        let segment_intersects_edges = horizontal_edges
            .iter()
            .all(|edge| edge.intersects(&segment));
        segment_intersects_edges
    }

    fn split_horizontally(&self, segment: EdgeF32) -> Option<(Trapezoid, Trapezoid)> {
        if self.segment_intersects_horizontal_edges(segment) {
            let horizontal_edges = self.horizontal_edges();
            let segment_top_first = segment.order_by_y();
            let segment_top = segment_top_first.0;
            let segment_bottom = segment_top_first.1;

            let segment_diff_y = segment_top.y - segment_bottom.y;

            let top_edge_y = horizontal_edges[0].0.y;
            let bottom_edge_y = horizontal_edges[1].0.y;

            let diff_y_bottom_edge = bottom_edge_y - segment_bottom.y;
            let diff_y_top_edge = top_edge_y - segment_bottom.y;

            let segment_dir = segment_top - segment_bottom;

            let segment_truncated = EdgeF32(
                segment_bottom + (diff_y_top_edge / segment_diff_y) * segment_dir,
                segment_bottom + (diff_y_bottom_edge / segment_diff_y) * segment_dir,
            );

            let left_trapezoid = Trapezoid {
                left: self.left,
                right: segment_truncated,
            };
            let right_trapezoid = Trapezoid {
                left: segment_truncated,
                right: self.right,
            };
            Some((left_trapezoid, right_trapezoid))
        } else {
            None
        }
    }
}

impl Map {
    pub fn new(islands: Vec<Vec<Point2<i32>>>) -> Self {
        let islands_triangulated = islands
            .iter()
            .map(|island| {
                let segments = island
                    .iter()
                    .enumerate()
                    .map(|(index, &vertex)| Edge(vertex, island[(index + 1) % island.len()]))
                    .collect::<VecDeque<_>>();

                // TODO: Use Point2<f32> for trapedoization and round back to Point2<i32>
                // Trapezoidation and Convert into query structure

                let mut trapezoids: HashMap<usize, Trapezoid> = [(
                    0,
                    Trapezoid {
                        left: EdgeF32(
                            Point2::new(-COORDINATE_MAX, COORDINATE_MAX),
                            Point2::new(-COORDINATE_MAX, -COORDINATE_MAX),
                        ),
                        right: EdgeF32(
                            Point2::new(COORDINATE_MAX, COORDINATE_MAX),
                            Point2::new(COORDINATE_MAX, -COORDINATE_MAX),
                        ),
                    },
                )]
                .iter()
                .cloned()
                .collect();
                let mut current_trapezoid_index = 1;

                let mut query_structure = QueryStructure {
                    root: QueryNode::Trapezoid(0),
                };

                let mut used_segments = VecDeque::<Edge>::new();

                for segment in segments.iter() {
                    let Edge(a, b) = segment.order_by_y();

                    let a_already_in_segments = used_segments.iter().any(|s| s.0 == a || s.1 == a);

                    if !a_already_in_segments {
                        let trapezoid_a_index = query_structure.query(a);
                        let trapezoid_a = trapezoids.get(&trapezoid_a_index).unwrap();
                        let (trapezoid_above, trapezoid_below) =
                            trapezoid_a.split_vertically(a.y as f32);
                        let trapezoid_above_index = current_trapezoid_index;
                        trapezoids.insert(trapezoid_above_index, trapezoid_above);
                        current_trapezoid_index += 1;

                        let trapezoid_below_index = current_trapezoid_index;
                        trapezoids.insert(trapezoid_below_index, trapezoid_below);
                        current_trapezoid_index += 1;

                        trapezoids.remove(&trapezoid_a_index);

                        query_structure.insert_y_node(
                            trapezoid_a_index,
                            trapezoid_above_index,
                            trapezoid_below_index,
                            a.y as f32,
                        );
                    }

                    let b_already_in_segments = used_segments.iter().any(|s| s.0 == b || s.1 == b);

                    if !b_already_in_segments {
                        let trapezoid_b_index = query_structure.query(b);
                        let trapezoid_b = trapezoids.get(&trapezoid_b_index).unwrap();
                        let (trapezoid_above, trapezoid_below) =
                            trapezoid_b.split_vertically(b.y as f32);

                        let trapezoid_above_index = current_trapezoid_index;
                        trapezoids.insert(trapezoid_above_index, trapezoid_above);
                        current_trapezoid_index += 1;

                        let trapezoid_below_index = current_trapezoid_index;
                        trapezoids.insert(trapezoid_below_index, trapezoid_below);
                        current_trapezoid_index += 1;

                        trapezoids.remove(&trapezoid_b_index);

                        query_structure.insert_y_node(
                            trapezoid_b_index,
                            trapezoid_above_index,
                            trapezoid_below_index,
                            b.y as f32,
                        );
                    }

                    for (&trapezoid_index, trapezoid) in trapezoids.clone().iter() {
                        if let Some((trapezoid_left, trapezoid_right)) = trapezoid
                            .split_horizontally(EdgeF32(to_f32(segment.0), to_f32(segment.1)))
                        {
                            let trapezoid_left_index = current_trapezoid_index;
                            trapezoids.insert(trapezoid_left_index, trapezoid_left);
                            current_trapezoid_index += 1;

                            let trapezoid_right_index = current_trapezoid_index;
                            trapezoids.insert(trapezoid_right_index, trapezoid_right);
                            current_trapezoid_index += 1;

                            trapezoids.remove(&trapezoid_index);

                            query_structure.insert_x_node(
                                trapezoid_index,
                                trapezoid_left_index,
                                trapezoid_right_index,
                                EdgeF32(to_f32(segment.0), to_f32(segment.1)),
                            );
                        }
                    }

                    used_segments.push_front(*segment);
                }

                let trapezoids_in_polygon = trapezoids
                    .iter()
                    .map(|(_, trapezoid)| trapezoid)
                    .filter(|trapezoid| {
                        trapezoid.vertices().iter().all(|&v| {
                            let crossings = segments
                                .iter()
                                .filter(|segment| {
                                    let segment_start = to_f32(segment.0);
                                    let segment_end = to_f32(segment.1);
                                    let segment_direction = segment_end - segment_start;
                                    let point = intersect_forgiving(
                                        v,
                                        Vector2::new(2.0 * COORDINATE_MAX, 0.0),
                                        segment_start,
                                        segment_direction,
                                    );

                                    if let Some(point) = point {
                                        if point.distance(&segment_start) < 0.001 {
                                            segment_end.y < segment_start.y
                                        } else if point.distance(&segment_end) < 0.001 {
                                            segment_start.y < segment_end.y
                                        } else {
                                            true
                                        }
                                    } else {
                                        false
                                    }
                                })
                                .count();

                            segments.iter().any(|segment| {
                                let segment_start = to_f32(segment.0);
                                let segment_end = to_f32(segment.1);
                                let segment_direction = segment_end - segment_start;
                                on_line(v, segment_start, segment_direction)
                            }) || crossings % 2 == 1
                        })
                    })
                    .collect::<Vec<_>>();

                // Decompose into monotone polygons
                let new_diagonals = trapezoids_in_polygon
                    .iter()
                    .filter_map(|trapezoid| {
                        let vertices = trapezoid.vertices();
                        // Edges are clockwise starting at top left
                        let trapezoid_edges = vertices
                            .iter()
                            .enumerate()
                            .map(|(index, &vertex)| {
                                EdgeF32(vertex, vertices[(index + 1) % vertices.len()])
                            })
                            .collect::<Vec<_>>();

                        let polygon_vertices_on_trapezoid_edges = island
                            .iter()
                            .map(|&v| {
                                let trapezoid_edges_v_is_on = trapezoid_edges
                                    .iter()
                                    .filter(|edge| {
                                        let segment_start = edge.0;
                                        let segment_end = edge.1;
                                        let segment_direction = segment_end - segment_start;
                                        on_line(to_f32(v), segment_start, segment_direction)
                                    })
                                    .collect::<Vec<_>>();
                                (to_f32(v), trapezoid_edges_v_is_on)
                            })
                            .filter(|(_, edges)| edges.len() > 0)
                            .collect::<Vec<_>>();

                        if polygon_vertices_on_trapezoid_edges.len() == 2 {
                            let no_shared_trapezoid_edge = polygon_vertices_on_trapezoid_edges[0]
                                .1
                                .iter()
                                .filter(|edge| {
                                    polygon_vertices_on_trapezoid_edges[1]
                                        .1
                                        .iter()
                                        .any(|other_edge| *edge == other_edge)
                                })
                                .count()
                                == 0;

                            if no_shared_trapezoid_edge {
                                let first_point = polygon_vertices_on_trapezoid_edges[0].0;
                                let second_point = polygon_vertices_on_trapezoid_edges[1].0;
                                Some(Edge(to_i32(first_point), to_i32(second_point)))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                println!("New diagonals {:?}", new_diagonals);

                let mut monotone_polygons: Vec<Vec<Point2<i32>>> = vec![island.to_vec()];
                // Split polygons with the new diagonal
                for diagonal in new_diagonals {
                    monotone_polygons = monotone_polygons
                        .into_iter()
                        .flat_map(|polygon| {
                            let diagonal_start = diagonal.0;
                            let diagonal_end = diagonal.1;
                            let polygon_contains_start =
                                polygon.iter().any(|&v| v == diagonal_start);
                            let polygon_contains_end = polygon.iter().any(|&v| v == diagonal_end);
                            if polygon_contains_start && polygon_contains_end {
                                let diagonal_start_index =
                                    polygon.iter().position(|&v| v == diagonal_start).unwrap();
                                let mut polygon_from_going_forwards = vec![];
                                for i in 0..polygon.len() {
                                    let new_vertex =
                                        polygon[(diagonal_start_index + i) % polygon.len()];
                                    polygon_from_going_forwards.push(new_vertex);
                                    if new_vertex == diagonal_end {
                                        break;
                                    }
                                }

                                let mut polygon_from_going_backwards = vec![];
                                for i in 0..polygon.len() {
                                    let new_vertex =
                                        polygon[(diagonal_start_index + polygon.len() - i)
                                            % polygon.len()];
                                    polygon_from_going_backwards.push(new_vertex);
                                    if new_vertex == diagonal_end {
                                        break;
                                    }
                                }
                                vec![polygon_from_going_forwards, polygon_from_going_backwards]
                            } else {
                                vec![polygon]
                            }
                        })
                        .collect::<Vec<_>>();
                }

                monotone_polygons
                    .into_iter()
                    .flat_map(|polygon| {
                        let polygon_segments = island
                            .iter()
                            .enumerate()
                            .map(|(index, &vertex)| {
                                Edge(vertex, island[(index + 1) % island.len()])
                            })
                            .collect::<Vec<_>>();

                        // Triangle, just return itself after making sure it's anticlockwise
                        if polygon.len() == 3 {
                            let clockwise = polygon
                                .iter()
                                .enumerate()
                                .map(|(i, &point)| {
                                    let next = polygon[(i + 1) % polygon.len()];
                                    (next.x - point.x) * (next.y + point.y)
                                })
                                .sum::<i32>()
                                >= 0;

                            return if clockwise {
                                vec![polygon[2], polygon[1], polygon[0]]
                            } else {
                                polygon
                            };
                        }

                        let (triangles, _) = polygon.iter().enumerate().fold(
                            (vec![], vec![]),
                            |(triangles, new_edges), (i, &point)| {
                                let prev_neighbour =
                                    polygon[(i + polygon.len() - 1) % polygon.len()];
                                let next_neighbour = polygon[(i + 1) % polygon.len()];
                                let adjacent_neighbours = vec![prev_neighbour, next_neighbour];

                                let neighbours = adjacent_neighbours
                                    .into_iter()
                                    .chain(
                                        new_edges
                                            .iter()
                                            .filter_map(|edge: &Edge| {
                                                if edge.0 == point {
                                                    Some(edge.1)
                                                } else if edge.1 == point {
                                                    Some(edge.0)
                                                } else {
                                                    None
                                                }
                                            })
                                            .collect::<Vec<Point2<i32>>>(),
                                    )
                                    .collect::<Vec<_>>();

                                let triangle_and_edge = neighbours
                                    .iter()
                                    .map(|&u| {
                                        neighbours
                                            .iter()
                                            .filter_map(|&w| {
                                                if w == u {
                                                    return None;
                                                };

                                                let new_edge = Edge(u, w);

                                                // Don't include itself
                                                let intersects_inner = new_edges
                                                    .iter()
                                                    .filter(|&other_edge| {
                                                        (new_edge.0 != other_edge.0
                                                            && new_edge.1 != other_edge.1)
                                                            && (new_edge.0 != other_edge.1
                                                                && new_edge.1 != other_edge.0)
                                                    })
                                                    .any(|&other_edge| {
                                                        new_edge.intersects(&other_edge)
                                                    });

                                                let intersects_outer = polygon_segments
                                                    .iter()
                                                    .any(|segment| segment.intersects(&new_edge));

                                                let inner_u = to_f32(u)
                                                    + 0.01 * (to_f32(w) - to_f32(u)).normalize();
                                                let inner_w = to_f32(w)
                                                    + 0.01 * (to_f32(u) - to_f32(w)).normalize();

                                                let inner_u_intersections_polygon =
                                                    polygon_segments
                                                        .iter()
                                                        .filter(|segment| {
                                                            let segment_start = to_f32(segment.0);
                                                            let segment_direction =
                                                                to_f32(segment.1)
                                                                    - to_f32(segment.0);
                                                            intersect(
                                                                inner_u,
                                                                Vector2::new(COORDINATE_MAX, 0.0),
                                                                segment_start,
                                                                segment_direction,
                                                            )
                                                        })
                                                        .collect::<Vec<_>>();

                                                let inner_u_within_polygon =
                                                    inner_u_intersections_polygon.len() % 2 == 1;

                                                let inner_w_within_polygon = polygon_segments
                                                    .iter()
                                                    .filter(|segment| {
                                                        let segment_start = to_f32(segment.0);
                                                        let segment_direction =
                                                            to_f32(segment.1) - to_f32(segment.0);
                                                        intersect(
                                                            inner_w,
                                                            Vector2::new(COORDINATE_MAX, 0.0),
                                                            segment_start,
                                                            segment_direction,
                                                        )
                                                    })
                                                    .count()
                                                    % 2
                                                    == 1;
                                                let within_polygon = inner_u_within_polygon
                                                    && inner_w_within_polygon;

                                                let triangle = [u, point, w];
                                                let duplicate_triangle = triangles.iter().any(
                                                    |&t: &[Point2<i32>; 3]| {
                                                        let t_set: HashSet<Point2<i32>> =
                                                            HashSet::from_iter(t.iter().cloned());

                                                        t_set
                                                            == HashSet::from_iter(
                                                                triangle.iter().cloned(),
                                                            )
                                                    },
                                                );

                                                if !intersects_outer
                                                    && !intersects_inner
                                                    && within_polygon
                                                    && !duplicate_triangle
                                                {
                                                    Some((triangle, new_edge))
                                                } else {
                                                    None
                                                }
                                            })
                                            .collect::<Vec<_>>()
                                    })
                                    .flatten()
                                    .next();

                                if let Some((triangle, new_edge)) = triangle_and_edge {
                                    let clockwise = triangle
                                        .iter()
                                        .enumerate()
                                        .map(|(i, &point)| {
                                            let next = triangle[(i + 1) % triangle.len()];
                                            (next.x - point.x) * (next.y + point.y)
                                        })
                                        .sum::<i32>()
                                        >= 0;
                                    let triangle = if clockwise {
                                        [triangle[2], triangle[1], triangle[0]]
                                    } else {
                                        triangle
                                    };
                                    (
                                        triangles
                                            .into_iter()
                                            .chain(vec![triangle])
                                            .collect::<Vec<_>>(),
                                        new_edges
                                            .into_iter()
                                            .chain(vec![new_edge])
                                            .collect::<Vec<_>>(),
                                    )
                                } else {
                                    (triangles, new_edges)
                                }
                            },
                        );

                        let triangles_combined: Vec<Point2<i32>> = triangles
                            .into_iter()
                            .map(|triangle| triangle.to_vec())
                            .flatten()
                            .collect::<Vec<_>>();
                        triangles_combined
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        Map {
            islands: islands_triangulated,
        }
    }
    pub fn into_vertices(&self) -> Vec<Vec<Position>> {
        self.islands
            .iter()
            .map(|island| {
                island
                    .iter()
                    .map(|&p| Position([p.x as f32, p.y as f32, 0.0]))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    }

    // TODO: Replace with just saving the input
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

    // TODO: Replace with just saving the input
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
