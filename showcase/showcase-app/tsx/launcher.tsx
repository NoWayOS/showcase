// The showcase launcher: the screen the Pi boots to, one tile per
// example kiosk, after the approved mockup (Main): a header with the
// demo line, a 3 x 2 grid of tiles - each with its status, its fictional
// brand, its title and one line of what it shows - and a footer. Every
// tile's title is a button, so the arrow keys and Tab reach each one: a
// ready example's opens it - the Service desk, the visitor sign-in
// and the pay at the pump - the fifth opens the Screen Compiler gallery,
// every component and style property the subset converts, and the sixth
// the "Why is this secure?" panel. The third is a program's own: a tile it may fill -
// its brand, title and line in slots, and a button that opens its example -
// hidden, taking no space, in a program that adds none.
// Every brand is fictional. Its header is a bar in the launcher's own brand
// colour; every text and size is here, easy to change.
import { Button, Column, HeaderBar, Icon, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const GAP_TILES = 16;
const EDGE = 16;

type LauncherProps = {
  onServiceDesk: () => void;
  onVisitor: () => void;
  extra: boolean;
  extraBrand: string;
  extraTitle: string;
  extraLine: string;
  onExtra: () => void;
  onPump: () => void;
  onGallery: () => void;
  onWhySecure: () => void;
};

export function LauncherScreen(props: LauncherProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <HeaderBar gap={GAP_TILES} padding={GAP}>
          <Icon name="house" />
          <Text id="title" role="heading">Choose a kiosk to try</Text>
          <Spacer />
          <Text>Demo: nothing kept</Text>
        </HeaderBar>
        <Text>React, run natively: no browser here</Text>
        <Row gap={GAP_TILES}>
          <Column gap={GAP}>
            <Text>Ready</Text>
            <Text>Kitchen sink</Text>
            <Button id="service-desk" onPress={props.onServiceDesk}>
              Service desk
            </Button>
            <Text>Every control</Text>
          </Column>
          <Column gap={GAP}>
            <Text>Ready</Text>
            <Text>Orrinvale</Text>
            <Button id="visitor" onPress={props.onVisitor}>
              Visitor sign-in
            </Button>
            <Text>Forgets you</Text>
          </Column>
          {props.extra && (
            <Column gap={GAP}>
              <Text>{props.extraBrand}</Text>
              <Text weight="bold">{props.extraTitle}</Text>
              <Button id="extra" onPress={props.onExtra}>
                Try it
              </Button>
              <Text>{props.extraLine}</Text>
            </Column>
          )}
        </Row>
        <Row gap={GAP_TILES}>
          <Column gap={GAP}>
            <Text>Ready</Text>
            <Text>Tarnwick Fuel</Text>
            <Button id="pump" onPress={props.onPump}>
              Pay at the pump
            </Button>
            <Text>Never sees a card</Text>
          </Column>
          <Column gap={GAP}>
            <Text>Ready</Text>
            <Text>Screen Compiler</Text>
            <Button id="gallery" onPress={props.onGallery}>
              React gallery
            </Button>
            <Text>Every component</Text>
          </Column>
          <Column gap={GAP}>
            <Text>Ready</Text>
            <Text>Under the glass</Text>
            <Button id="why-secure" onPress={props.onWhySecure}>
              Why is this secure?
            </Button>
            <Text>Built or simulated</Text>
          </Column>
        </Row>
        <Spacer />
        <Text>Fictional brands. Arrows or Tab move, Enter chooses.</Text>
      </Column>
    </Screen>
  );
}
