use candle_core::{DType, Device, IndexOp, Result, Tensor};
use candle_nn::{
    Activation, AdamW, Module, Sequential, VarBuilder, VarMap, linear,
    ops::{log_softmax, softmax},
    seq,
};
use rand::{
    Rng,
    distr::weighted::WeightedIndex,
    rngs::ThreadRng,
    seq::{IteratorRandom, index::sample_weighted},
};

use crate::game::{
    GameContext, GameState, PlayerInputs,
    ai::{ACTION_SPACE, STATE_VECTOR_LEN, serialize_observation},
};

const AGENT1_OUTPUT_PATH: &str = "./resources/scenes/dqn_agent1_weights_NEW.safetensors";
const AGENT2_OUTPUT_PATH: &str = "./resources/scenes/dqn_agent2_weights_NEW.safetensors";
const SAVE_INTERVAL: usize = 5000;

const HIDDEN_COUNT: usize = 256;
const LEARNING_RATE: f64 = 0.0001;

struct ActorCritic {
    actor: Sequential,
    critic: Sequential,
}

impl ActorCritic {
    fn new(var_map: &VarMap, device: &Device) -> Result<Self> {
        let vb = VarBuilder::from_varmap(&var_map, DType::F32, device);
        Ok(Self {
            actor: seq()
                .add(linear(STATE_VECTOR_LEN, HIDDEN_COUNT, vb.pp("linear_in"))?)
                .add(Activation::Sigmoid)
                .add(linear(HIDDEN_COUNT, HIDDEN_COUNT, vb.pp("hidden"))?)
                .add(Activation::Sigmoid)
                .add(linear(HIDDEN_COUNT, ACTION_SPACE, vb.pp("linear_out"))?),
            critic: seq()
                .add(linear(STATE_VECTOR_LEN, HIDDEN_COUNT, vb.pp("linear_in"))?)
                .add(Activation::Sigmoid)
                .add(linear(HIDDEN_COUNT, HIDDEN_COUNT, vb.pp("hidden"))?)
                .add(Activation::Sigmoid)
                .add(linear(HIDDEN_COUNT, 1, vb.pp("linear_out"))?),
        })
    }

    fn act(&self, obs: &Tensor, rng: &mut rand::rngs::ThreadRng) -> Result<(usize, f32, f32)> {
        let estimates = self.actor.forward(&obs.unsqueeze(0)?)?;
        let action_probs = softmax(&estimates.squeeze(0)?, 0)?;
        let weights = action_probs.to_vec1::<f32>()?;
        let action = rng.sample(WeightedIndex::new(weights).unwrap());

        let state_val = self.critic.forward(obs)?.to_scalar()?;

        let action_logprob = action_probs.i(action)?.to_scalar::<f32>()?.ln();

        Ok((action, action_logprob, state_val))
    }

    fn evaluate(&self, states: &Tensor, actions: &Tensor) -> Result<(Tensor, Tensor, Tensor)> {
        let logits = self.actor.forward(states)?;
        let probs = softmax(&logits, 1)?;
        let log_probs = probs.log()?;

        let action_logprobs = log_probs.gather(actions, 1)?;
        let entropy = (probs * log_probs)?.sum_keepdim(1)?.neg()?;
        let state_val = self.critic.forward(states)?;

        Ok((action_logprobs, state_val, entropy))
    }
}

fn train(context: &GameContext, inputs: &mut PlayerInputs, state: &mut GameState) -> Result<()> {
    let device = Device::Cpu;

    let var_map1 = VarMap::new();
    let var_map2 = VarMap::new();
    let agent1 = ActorCritic::new(&var_map1, &device)?;
    let agent1 = ActorCritic::new(&var_map2, &device)?;

    let mut optimizer1 = AdamW::new_lr(var_map1.all_vars(), LEARNING_RATE)?;
    let mut optimizer2 = AdamW::new_lr(var_map2.all_vars(), LEARNING_RATE)?;

    Ok(())
}

fn get_ai_action(old_policy: &ActorCritic, obs: &Tensor, rng: &mut ThreadRng) -> Result<()> {
    let (action, action_prob, state_val) = old_policy.act(&obs, rng)?;
    Ok(())
}
