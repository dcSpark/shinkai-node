use shinkai_vector_resources::{
    graph_resource::{GraphEdge, GraphRagResource},
    vector_resource::Node,
};
use uuid::Uuid;

#[test]
fn graph_resource_test() {
    let node_a = Node::new_text(Uuid::new_v4().to_string(), "Node A".to_string(), None, &vec![]);
    let node_b = Node::new_text(Uuid::new_v4().to_string(), "Node B".to_string(), None, &vec![]);
    let node_c = Node::new_text(Uuid::new_v4().to_string(), "Node C".to_string(), None, &vec![]);
    let node_d = Node::new_text(Uuid::new_v4().to_string(), "Node D".to_string(), None, &vec![]);

    let mut edges = Vec::new();
    edges.push(GraphEdge {
        source: node_a.id.clone(),
        target: node_b.id.clone(),
        weight: 1.0,
    });
    edges.push(GraphEdge {
        source: node_a.id.clone(),
        target: node_c.id.clone(),
        weight: 1.0,
    });
    edges.push(GraphEdge {
        source: node_c.id.clone(),
        target: node_d.id.clone(),
        weight: 1.0,
    });

    let graph_resource = GraphRagResource::new(
        "Test Graph".to_string(),
        None,
        vec![node_a, node_b, node_c, node_d],
        edges,
    );

    assert_eq!(graph_resource.node_count(), 4);
    assert_eq!(graph_resource.edge_count(), 3);
}
