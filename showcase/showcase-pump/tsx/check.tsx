// Pay at the pump, check your choices: the pump, fuel and amount
// from the UI's own copies, then pay on the card reader - or, for a
// demonstration, have the card declined. Fictional brand; every text
// invented.
import { Button, Card, Column, Notice, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const GAP_ACTIONS = 12;
const EDGE = 24;
const TIGHT = 12;

type CheckProps = {
  pump: string;
  fuel: string;
  amount: string;
  onBack: () => void;
  onCancel: () => void;
  onDecline: () => void;
  onPay: () => void;
};

export function CheckScreen(props: CheckProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="heading" role="title">
          Check and pay
        </Text>
        <Card gap={GAP} padding={TIGHT}>
          <Row gap={GAP}>
            <Text role="label">Pump</Text>
            <Text weight="bold">{props.pump}</Text>
          </Row>
          <Row gap={GAP}>
            <Text role="label">Fuel</Text>
            <Text weight="bold">{props.fuel}</Text>
          </Row>
          <Row gap={GAP}>
            <Text role="label">Up to</Text>
            <Text weight="bold">{props.amount}</Text>
          </Row>
        </Card>
        <Notice tone="caution" padding={TIGHT}>
          <Text weight="bold">Next, pay on the card reader. Follow the instructions on the card reader.</Text>
        </Notice>
        <Spacer />
        <Row gap={GAP_ACTIONS}>
          <Button variant="secondary" onPress={props.onBack}>
            Change
          </Button>
          <Button variant="quiet" onPress={props.onCancel}>
            Cancel
          </Button>
          <Spacer />
          <Button id="decline" variant="secondary" onPress={props.onDecline}>
            Demo: card declined
          </Button>
          <Button id="pay" onPress={props.onPay}>
            Pay on the card reader
          </Button>
        </Row>
      </Column>
    </Screen>
  );
}
