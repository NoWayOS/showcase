// Kiosk reference screen, phase Review: every answer, from the UI's
// own copies (the app-UI contract sends no answers back), before sending.
// Synthetic and sector-neutral; not taken from any product or customer.
import { Button, Column, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const GAP_ACTIONS = 16;
const EDGE = 32;

type ReviewProps = {
  name: string;
  reference: string;
  topic: string;
  details: string;
  receipt: string;
  onBack: () => void;
  onCancel: () => void;
  onSend: () => void;
};

export function ReviewScreen(props: ReviewProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="heading">Check your answers</Text>
        <Row gap={GAP}>
          <Text>Name:</Text>
          <Text>{props.name}</Text>
        </Row>
        <Row gap={GAP}>
          <Text>Reference number:</Text>
          <Text>{props.reference}</Text>
        </Row>
        <Row gap={GAP}>
          <Text>Topic:</Text>
          <Text>{props.topic}</Text>
        </Row>
        <Row gap={GAP}>
          <Text>Details:</Text>
          <Text>{props.details}</Text>
        </Row>
        <Row gap={GAP}>
          <Text>Receipt:</Text>
          <Text>{props.receipt}</Text>
        </Row>
        <Spacer />
        <Row gap={GAP_ACTIONS}>
          <Button onPress={props.onBack}>Change</Button>
          <Button onPress={props.onCancel}>Cancel</Button>
          <Spacer />
          <Button id="send" onPress={props.onSend}>
            Send
          </Button>
        </Row>
      </Column>
    </Screen>
  );
}
