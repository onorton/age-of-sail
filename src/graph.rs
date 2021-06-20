use amethyst::core::{alga::linear::EuclideanSpace, math::Point2};
use priority_queue::PriorityQueue;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Copy)]
pub struct Edge(pub usize, pub usize);

pub struct Graph {
    pub nodes: Vec<Point2<f32>>,
    pub edges: Vec<Edge>,
}

impl Graph {
    pub fn a_star(&self, start: usize, end: usize) -> Vec<Point2<f32>> {
        let mut frontier = PriorityQueue::new();
        frontier.push(start, 0);
        let mut came_from = HashMap::<usize, Option<usize>>::new();
        came_from.insert(start, None);
        let mut cost_so_far = HashMap::<usize, f32>::new();
        cost_so_far.insert(start, 0.0);

        while !frontier.is_empty() {
            let current = frontier.pop().unwrap().0;

            if current == end {
                break;
            }

            for next in self.neighbours(current) {
                let new_cost = cost_so_far[&current] + self.cost(current, next);
                if !cost_so_far.contains_key(&next) || new_cost < cost_so_far[&next] {
                    cost_so_far.insert(next, new_cost);
                    let priority = new_cost + self.cost(end, next);
                    frontier.push(next, -priority as i32);
                    came_from.insert(next, Some(current));
                }
            }
        }

        let mut nodes = VecDeque::new();
        let mut previous = Some(end);
        while previous.is_some() {
            nodes.push_front(previous.unwrap());
            previous = came_from[&previous.unwrap()];
        }
        nodes.pop_front();

        nodes.iter().map(|&n| self.nodes[n]).collect::<Vec<_>>()
    }

    fn neighbours(&self, node_index: usize) -> HashSet<usize> {
        self.edges
            .iter()
            .filter_map(|edge| {
                let index = if edge.0 == node_index {
                    Some(edge.1)
                } else if edge.1 == node_index {
                    Some(edge.0)
                } else {
                    None
                };
                index
            })
            .collect::<HashSet<_>>()
    }

    fn cost(&self, current: usize, next: usize) -> f32 {
        self.nodes[current].distance(&self.nodes[next])
    }
}
