// Kiosk reference screen, phase Step, a choice field: one topic out
// of three, required, with its error beside it, and optional details.
// Synthetic and sector-neutral; not taken from any product or customer.
import { Button, Column, RadioGroup, Row, Screen, Spacer, Text, TextInput } from "nowayos:kit";

const GAP = 8;
const GAP_ACTIONS = 16;
const EDGE = 32;

type QuestionProps = {
  topicError: string;
  onBack: () => void;
  onCancel: () => void;
  onNext: () => void;
};

export function QuestionStep(props: QuestionProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="heading">What is your question about?</Text>
        <RadioGroup id="topic" options={["Opening hours", "An order", "Something else"]} />
        {props.topicError && <Text>{props.topicError}</Text>}
        <Row gap={GAP}>
          <Text>Details (optional)</Text>
          <TextInput id="details" placeholder="A few words" maxLength={48} />
        </Row>
        <Spacer size={GAP_ACTIONS} />
        <Row gap={GAP_ACTIONS}>
          <Button onPress={props.onBack}>Back</Button>
          <Button onPress={props.onCancel}>Cancel</Button>
          <Spacer />
          <Button id="next" onPress={props.onNext}>
            Next
          </Button>
        </Row>
      </Column>
    </Screen>
  );
}
