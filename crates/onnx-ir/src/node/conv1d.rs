use crate::ir::Node;

use super::padding::{PaddingConfig1d, padding_config_1d};

/// Configuration for Conv1d operations extracted from ONNX nodes
#[derive(Debug, Clone)]
pub struct Conv1dConfig {
    /// Input channels
    pub channels_in: usize,
    /// Output channels
    pub channels_out: usize,
    /// Kernel size
    pub kernel_size: usize,
    /// Stride
    pub stride: usize,
    /// Dilation
    pub dilation: usize,
    /// Number of groups
    pub groups: usize,
    /// Whether bias is used
    pub bias: bool,
    /// Padding configuration
    pub padding: PaddingConfig1d,
}

impl Conv1dConfig {
    /// Create a new Conv1dConfig
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        channels_in: usize,
        channels_out: usize,
        kernel_size: usize,
        stride: usize,
        padding: PaddingConfig1d,
        dilation: usize,
        groups: usize,
        bias: bool,
    ) -> Self {
        Self {
            channels_in,
            channels_out,
            kernel_size,
            stride,
            padding,
            dilation,
            groups,
            bias,
        }
    }
}

/// Create a Conv1dConfig from the attributes of the node
pub fn conv1d_config(curr: &Node) -> Conv1dConfig {
    let mut kernel_shape = Vec::new(); // TODO default inferred from weight tensor per spec
    let mut strides = vec![1];
    let mut pads = vec![0, 0];
    let mut dilations = vec![1];
    let mut group: usize = 1;

    let weight_shape = curr.inputs[1]
        .value
        .as_ref()
        .expect("Conv1d: weight tensor must be present")
        .shape
        .clone();

    // check if the bias is present
    let bias = curr.inputs.len() == 3;

    for (key, value) in curr.attrs.iter() {
        match key.as_str() {
            "kernel_shape" => kernel_shape = value.clone().into_i64s(),
            "strides" => strides = value.clone().into_i64s(),
            "pads" => pads = value.clone().into_i64s(),
            "dilations" => dilations = value.clone().into_i64s(),
            "group" => group = value.clone().into_i64() as usize,
            _ => {}
        }
    }

    // the channels are inverted in the weight tensor
    let channels_in = weight_shape[1] * group;
    let channels_out = weight_shape[0];

    let padding = padding_config_1d(&pads);

    Conv1dConfig {
        channels_in,
        channels_out,
        kernel_size: kernel_shape[0] as usize,
        stride: strides[0] as usize,
        dilation: dilations[0] as usize,
        groups: group,
        bias,
        padding,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::NodeType;
    use crate::node::test_utils::NodeBuilder;

    fn create_test_node(
        kernel_shape: Vec<i64>,
        strides: Vec<i64>,
        pads: Vec<i64>,
        dilations: Vec<i64>,
        group: i64,
        has_bias: bool,
    ) -> Node {
        // Create weight tensor data
        let weight_data = vec![0.1; 16];

        // Start building the node with input and weight
        let mut builder = NodeBuilder::new(NodeType::Conv1d, "test_conv1d")
            .input_tensor_f32("data", 3, None)
            .input_tensor_f32_data(
                "weight",
                weight_data,
                vec![2, 2, 4], // [out_channels, in_channels, kernel_size]
            )
            .output_tensor_f32("output", 3, None);

        // Add bias if needed
        if has_bias {
            builder = builder.input_tensor_f32_data("bias", vec![0.1, 0.2], vec![2]);
        }

        // Add attributes
        builder = builder
            .attr_ints("kernel_shape", kernel_shape)
            .attr_ints("strides", strides)
            .attr_ints("pads", pads)
            .attr_ints("dilations", dilations)
            .attr_int("group", group);

        builder.build()
    }

    #[test]
    fn test_conv1d_config_basic() {
        let node = create_test_node(vec![4], vec![1], vec![0, 0], vec![1], 1, false);
        let config = conv1d_config(&node);

        assert_eq!(config.channels_in, 2);
        assert_eq!(config.channels_out, 2);
        assert_eq!(config.kernel_size, 4);
        assert_eq!(config.stride, 1);
        assert_eq!(config.dilation, 1);
        assert_eq!(config.groups, 1);
        assert!(!config.bias);
        assert!(matches!(config.padding, PaddingConfig1d::Valid));
    }

    #[test]
    fn test_conv1d_config_with_padding() {
        let node = create_test_node(vec![4], vec![2], vec![2, 2], vec![1], 1, true);
        let config = conv1d_config(&node);

        assert_eq!(config.channels_in, 2);
        assert_eq!(config.channels_out, 2);
        assert_eq!(config.kernel_size, 4);
        assert_eq!(config.stride, 2);
        assert_eq!(config.dilation, 1);
        assert_eq!(config.groups, 1);
        assert!(config.bias);
        assert!(matches!(config.padding, PaddingConfig1d::Explicit(2)));
    }

    #[test]
    fn test_conv1d_config_with_dilation() {
        let node = create_test_node(vec![4], vec![1], vec![0, 0], vec![2], 1, false);
        let config = conv1d_config(&node);

        assert_eq!(config.channels_in, 2);
        assert_eq!(config.channels_out, 2);
        assert_eq!(config.kernel_size, 4);
        assert_eq!(config.stride, 1);
        assert_eq!(config.dilation, 2);
        assert_eq!(config.groups, 1);
        assert!(!config.bias);
        assert!(matches!(config.padding, PaddingConfig1d::Valid));
    }

    #[test]
    fn test_conv1d_config_with_groups() {
        let node = create_test_node(vec![4], vec![1], vec![0, 0], vec![1], 2, false);
        let config = conv1d_config(&node);

        assert_eq!(config.channels_in, 4);
        assert_eq!(config.channels_out, 2);
        assert_eq!(config.kernel_size, 4);
        assert_eq!(config.stride, 1);
        assert_eq!(config.dilation, 1);
        assert_eq!(config.groups, 2);
        assert!(!config.bias);
        assert!(matches!(config.padding, PaddingConfig1d::Valid));
    }

    #[test]
    #[should_panic(expected = "Asymmetric padding is not supported")]
    fn test_conv1d_config_asymmetric_padding() {
        let node = create_test_node(vec![4], vec![1], vec![1, 2], vec![1], 1, false);
        let _ = conv1d_config(&node);
    }

    #[test]
    #[should_panic(expected = "Negative pad values are not supported")]
    fn test_conv1d_config_negative_padding() {
        let node = create_test_node(vec![4], vec![1], vec![-1, -1], vec![1], 1, false);
        let _ = conv1d_config(&node);
    }
}
