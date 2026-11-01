// Kiosk reference screen, phase Step, a step with no fields: a
// notice the person reads before going on.
// Synthetic and sector-neutral; not taken from any product or customer.
import { Button, Column, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const GAP_ACTIONS = 16;
const EDGE = 32;

type NoticeProps = {
  onBack: () => void;
  onCancel: () => void;
  onNext: () => void;
};

export function NoticeStep(props: NoticeProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="heading">Before you send</Text>
        <Text>Your answers are used only to reply to you.</Text>
        <Text>Nothing you enter is kept on this kiosk.</Text>
        <Spacer />
        <Row gap={GAP_ACTIONS}>
          <Button onPress={props.onBack}>Back</Button>
          <Button onPress={props.onCancel}>Cancel</Button>
          <Spacer />
          <Button id="next" onPress={props.onNext}>
            Continue
          </Button>
        </Row>
      </Column>
    </Screen>
  );
}
